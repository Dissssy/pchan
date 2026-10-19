// Receives Web Push messages and turns them into OS notifications.
//
// There is deliberately no fetch handler and no offline cache: the app is served fresh from
// the backend, and the old precache here was never read back.

// Take over from an older worker as soon as this one installs. Without these, a changed
// worker waits for every tab to close, so fixes here would not reach anyone for days.
self.addEventListener('install', function () {
  self.skipWaiting();
});

self.addEventListener('activate', function (e) {
  e.waitUntil(Promise.all([
    self.clients.claim(),
    caches.delete('yew-pwa'),
  ]));
});

function threadUrl(board, number) {
  return new URL('/' + encodeURIComponent(board) + '/thread/' + encodeURIComponent(number), self.location.origin);
}

// Compare pages by path alone: a trailing slash or a #post anchor is still the same thread.
function samePage(clientUrl, url) {
  const strip = function (p) { return p.length > 1 ? p.replace(/\/+$/, '') : p; };
  return strip(new URL(clientUrl).pathname) === strip(url.pathname);
}

self.addEventListener('push', function (e) {
  e.waitUntil(showForPush(e));
});

async function showForPush(e) {
  // The server wraps the details as JSON inside `body`. A message that is missing or
  // malformed still has to produce a notification: a push subscription is created with
  // userVisibleOnly, and the browser punishes a push that shows nothing.
  let raw = {};
  let body = {};
  try {
    raw = e.data ? e.data.json() : {};
    body = typeof raw.body === 'string' ? JSON.parse(raw.body) : (raw.body || {});
  } catch (err) {
    console.error('unreadable push payload', err);
  }

  const title = raw.title || 'New post';
  const options = {
    icon: body.thumbnail || 'res/icon-256.png',
    vibrate: [100, 50, 100],
    actions: [
      { action: 'open', title: 'Open thread', icon: 'res/confirm.png' },
      { action: 'close', title: 'Go away.', icon: 'res/decline.png' },
    ],
  };

  if (body.board_discriminator && body.thread_post_number !== undefined) {
    const url = threadUrl(body.board_discriminator, body.thread_post_number);
    // One notification per thread: the next post replaces the last instead of stacking a
    // pile of them, and still alerts.
    // A reply to the member's own post is not folded into the thread's notification, which
    // would hide it under a newer, less important one.
    options.tag = body.reply
      ? 'reply:' + body.board_discriminator + ':' + body.post_number
      : 'thread:' + body.board_discriminator + ':' + body.thread_post_number;
    options.renotify = true;
    // Opens at the post that was made, using the page's own #post highlighting.
    const target = new URL(url.href);
    if (body.post_number !== undefined) {
      target.hash = String(body.post_number);
    }
    // What "Go away." needs to mark the thread read without opening the site.
    options.data = {
      url: target.href,
      board: body.board_discriminator,
      thread: body.thread_post_number,
      post: body.post_number,
    };
    options.body = (body.author || 'Someone') + ' posted in ' + (body.thread_topic || 'a thread')
      + (body.content ? '\n' + body.content : '');

    // Someone with the thread on screen sees the post appear live; a notification on top of
    // that is only noise. Visible, not focused: a window beside the one being typed in is
    // still being looked at.
    const windows = await self.clients.matchAll({ type: 'window', includeUncontrolled: true });
    const watching = windows.some(function (c) {
      return c.visibilityState === 'visible' && samePage(c.url, url);
    });
    if (watching) {
      return;
    }
  } else {
    options.data = { url: new URL('/', self.location.origin).href };
  }

  await self.registration.showNotification(title, options);
}

self.addEventListener('notificationclick', function (event) {
  event.notification.close(); // Android needs this to be explicit.
  if (event.action === 'close') {
    // An explicit "Go away." means the member has dealt with it: mark the thread read, so it
    // leaves the inbox on every device. Swiping the notification away is not the same thing
    // (it can also just be the system clearing it), so that changes nothing.
    const data = event.notification.data || {};
    if (data.board !== undefined && data.thread !== undefined && data.post !== undefined) {
      event.waitUntil(
        fetch('/api/v1/board/' + encodeURIComponent(data.board) + '/thread/' + encodeURIComponent(data.thread) + '/read', {
          method: 'PUT',
          headers: { 'Content-type': 'application/json' },
          body: JSON.stringify({ through: data.post }),
          credentials: 'same-origin',
        }).catch(function () { })
      );
    }
    return;
  }

  const target = new URL((event.notification.data && event.notification.data.url) || '/', self.location.origin);
  event.waitUntil(
    self.clients.matchAll({ type: 'window', includeUncontrolled: true }).then(function (windows) {
      // Reuse a tab that already has the thread open instead of opening another.
      for (const client of windows) {
        if (samePage(client.url, target) && 'focus' in client) {
          // Already open, and the post is on screen live: just bring it forward.
          return client.focus();
        }
      }
      return self.clients.openWindow(target.href);
    })
  );
});
