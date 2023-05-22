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
self.addEventListener('fetch', function(e) {
  // if request url ends with notification, do not intercept as it is an sse request and should not be cached
  if (!e.request.url.endsWith("notifications")) {
    e.respondWith(
      caches.match(e.request).then(function(response) {
        return response || fetch(e.request);
      })
    );
  } else {
    console.log("not intercepting sse request: " + e.request.url);
  }
});