# pixfer

so pixfer is a way to move files between 2 devices using only light. one device flashes data out of its screen and the other device recieves it with its camera. no bluetooth. no wifi. no gps. nothing else. just light and the parts every phone already has.

how it works

the sending side renders a grid of blocks on its screen, kind of like a qr code but its live and it changes every frame. the receiving side points its camera at that screen and reads the blocks. the heavy lifting — the protocol, the error correction, the chunking — is all rust. the web demo lets u open 2 browser tabs on 2 devices, one tab sends and the other recieves, no installs needed.

the repo

- pixfer-core: rust core for the protocol, block encoding and reed solomon error correction, built to compile to wasm for the web demo
- the web demo: tx tab with a canvas light + rx tab with camera capture, pure browser
- RESEARCH.md and AGENTS.md live on disk only, they are gitignored so they are never commited

status

the rust core is the next build target. after that the web demo goes up so u can point a phone camera at a laptop screen and watch a file move through light.

the repo is public so feel free to fork it or open an issue if ur testing the link and something breaks.