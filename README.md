# pixfer

so pixfer is a way to move files between 2 devices using only light one device flashes data out of its screen and the other recieves it with its camera no bluetooth no wifi no gps nothing else just light and the parts every phone already has

how it works

the sending side flashes actual qr codes on its screen and the receiving side points its camera at that screen and reads them. the heavy lifting like the protocol and error correction and chunking lives in a rust core and the web demo is a plain js mirror of it so u can open 2 browser tabs on 2 devices where one tab sends and the other recieves with no installs no build step and no dependencies except a 32kb zip library

the transfer is a loop not a stop and wait. the sender round robins a ping frame then every chunk frame then ping again forever, each qr is on screen for a fixed dwell and the sender never waits for an ack. every qr carries its own frame number (chunk index + total) in its header so the receiver always knows exactly which frames its missing and just waits for the loop to bring them around. lost a frame to a shaky hand or a blur or the code sliding off screen? it comes back on the next pass. when the receiver has every chunk it flashes a done qr back and the sender stops.

the frames

- every frame is a real qr code at error correction level m, the same format a phone scanner reads, except the dark modules are painted in a rotating 8 color palette. the colors carry no data, only the dark/light classification matters, they just make the flashing readable to ur eye. all palette colors are luminance-dark and strongly saturated so a camera still classifies them dark after white balance and gain shifts
- each qr holds a 23 byte header (magic "PIXF", frame kind, session id, chunk index, total chunks, payload length, crc32) plus the payload. control frames (ping pong done) are tiny v2 codes, chunk frames are v11 by default
- the size selector on the sender picks the qr version: standard is v11 with 223 bytes per frame, big is v20 with 643, max is v25 with 974. bigger frames mean fewer frames for the same file but the modules get smaller so the receiver has to be closer and steadier. u can change it mid transfer, the loop restarts clean and the receiver resets on the next ping
- the flash is 2 phases, normal (colored modules on white) and inverted (white modules on black), 200ms per phase by default. the blink gives the camera a strong signal and u can slow it down from the flash box on the sender if ur camera is struggling

error correction is layered. the qr itself is EC level m which corrects about 15 bad bytes per block, on top of that every frame has a crc32 so a "successful" decode can never be silently wrong, and the whole thing is wrapped in the numbered-frame loop so anything that dies entirely just comes back around

the decoder

this is where most of the interesting stuff lives, its in web/js/qrdecode.js and it has to survive real phone cameras which are awful

- finder detection runs a flood fill over the frame and picks the three blobs that form a tight right-angle corner, geometry not blob size, because data blobs love to cosplay as finder rings
- the module grid is affine, derived from the three finder centers, so a few degrees of hand tilt decode exactly instead of smearing
- the frame is read at multiple scales, ÷4 first (fast), then ÷2, then full resolution if the stream is small enough, and for 1080p phones where full res is too big there is a centered square crop at native resolution. downsampling is done in chained smoothed 2x steps because one big nearest-neighbor decimation aliases the sending screen's pixel grid into garbage
- format words are matched against every valid candidate within 3 bit errors so a noisy capture still resolves the mask, and the inverted flash phase is detected from the same word
- a warm start caches the last good grid per buffer size and tries it before the expensive flood fill, steady phones skip detection entirely
- the receiver measures its true frame rate and if the camera is slow (phones drop to 10-15fps in dim rooms) the status pill names the exact flash value to type on the sender, like "camera only ~5 fps — set flash to 800 ms"

the receiver also has adaptive decode throttling (slow phones sample fewer frames instead of backing up the camera loop), a screen wake lock so phones dont dim mid transfer, and an exposure lock that freezes at the current exposure so the flashing screen stops the auto exposure from hunting

how to run

the web demo needs a camera and browsers only give camera access on https or localhost, that part is not optional

quick local

```
cd web
npx serve .
# or: python3 -m http.server
```

open http://localhost:3000 (or whatever port it prints) on two devices or two tabs. pick send on one, receive on the other. u can also jump straight to a view with ?mode=tx or ?mode=rx in the url

for two real phones either deploy it or use a tunnel like cloudflared or ngrok pointing at ur local server, https included, then open the link on both phones

deploy

the demo is fully static, any static host works. its currently on vercel:

```
cd web
vercel deploy --prod
```

theres no build step and no npm install needed for the app itself, only for the tests

tests

```
cd web
npm test
```

runs all 54 tests with node --test: protocol round trips, qr encode/decode through pixels, reed-solomon correction of corrupted modules, edge cases (short last chunks land on every qr version from v2 to v11, hostile frames get rejected), a module smoke test that actually mounts both views under a fake dom so a runtime bug cant ship, and the best part, a camera simulation that renders frames through gaussian blur, sensor noise, white balance shift, hand shake, rotation wobble, focus breathing, exposure blending and scripted off-screen excursions at 30fps, then proves the loop recaptures every lost chunk by frame number

the rust core

```
cargo test --workspace --locked
```

pixfer-core is the reference implementation, the web demo mirrors the parts it needs. note cargo test --all-targets currently fails on the criterion bench file, plain cargo test is the one that runs clean

project layout

- pixfer-core/ — the rust crate, protocol, gf(256), rs(255,223), sessions, transfer logic, plus a lot of experimental modules
- web/ — the demo, no framework, plain es modules
  - js/core.js — the protocol mirror, frame building, crc32, rs, chunk metadata validation
  - js/qr.js — a real qr encoder, versions 1-40, EC-M, bch15/18, penalty mask selection
  - js/qrdecode.js — the camera decoder described above
  - js/light.js — the emitter (canvas painting, flash phases, wake lock) and camera opening
  - js/tx.js / rx.js — the two views
  - tests/ — the node --test suite including the camera simulation

practical notes

- keep both screens awake and bright, hold the phones 10-20cm apart in a well lit room
- if the receiver says "camera only ~N fps" type the number it names into the sender's flash box, thats the whole fix
- if the preview pulses bright and dark, tap lock exposure on the receiver
- start with the standard 223 byte frames, they are the reliable default. big and max need a steadier closer capture and its a real trade not a free lunch
