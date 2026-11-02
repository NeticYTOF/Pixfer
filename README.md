# pixfer

so pixfer is a way to move files between 2 devices using only light one device flashes data out of its screen and the other recieves it with its camera no bluetooth no wifi no gps nothing else just light and the parts every phone already has

how it works

the sending side renders a grid of blocks on its screen kind of like a qr code but its live and changes every frame the receiving side points its camera at that screen and reads the blocks the heavy lifting like the protocol and error correction and chunking is all rust and the web demo lets u open 2 browser tabs on 2 devices where one tab sends and the other recieves with no installs