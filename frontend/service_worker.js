var cacheName = 'yew-pwa';
var filesToCache = [
  './index.html',
  './frontend.js',
  './frontend_bg.wasm',
  './res/',
];


/* Start the service worker and cache all of the app's content */
self.addEventListener('install', function(e) {
  e.waitUntil(
    caches.open(cacheName).then(function(cache) {
      return cache.addAll(filesToCache);
    })
  );
});

/* Serve cached content when offline */
// self.addEventListener('fetch', function(e) {
//   // if request url ends with notification, do not intercept as it is an sse request and should not be cached
//   if (!e.request.url.endsWith("notifications")) {
//     e.respondWith(
//       caches.match(e.request).then(function(response) {
//         return response || fetch(e.request);
//       })
//     );
//   } else {
//     // console.log("not intercepting sse request: " + e.request.url);
//   }
// });

self.addEventListener('push', (e) => {
  let raw_body = e.data.json();
  let body = JSON.parse(raw_body.body);
  let final_body = body.author + " posted in " + body.thread_topic + "\n" + body.content;
  const options = {
    body: final_body,
    icon: body.thumbnail || 'res/icon-256.png',
    vibrate: [100, 50, 100],
    data: {
      dateOfArrival: Date.now(),
      primaryKey: 1,
      url: 'https://pchan.p51.nl/' + body.board_discriminator + '/thread/' + body.thread_post_number,
    },
    actions: [
      {
        action: 'open',
        title: 'Open thread',
        icon: 'res/confirm.png',
      },
      {
        action: 'close',
        title: "Go away.",
        icon: 'res/decline.png',
      },
    ],
  };
  e.waitUntil(self.registration.showNotification(raw_body.title, options));
});

self.addEventListener('notificationclick', (event) => {
  const eventAction = "" + event.action;

  if (eventAction == 'close') {
    return;
  }

  const url = event.notification.data.url;
  event.notification.close(); // Android needs explicit close.
  event.waitUntil(
    clients.matchAll({ type: 'window' }).then((windowClients) => {
      // Check if there is already a window/tab open with the target URL
      // for (var i = 0; i < windowClients.length; i++) {
      //   var client = windowClients[i];
      //   // If so, just focus it.
      //   if (client.url === url && 'focus' in client) {
      //     return client.focus();
      //   }
      // }
      // If not, then open the target URL in a new window/tab.
      if (clients.openWindow) {
        return clients.openWindow(url);
      }
    })
  );
});