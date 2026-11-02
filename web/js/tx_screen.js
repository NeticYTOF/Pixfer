// pixfer web transmitter screen — renders block grids to the display.
// Mirrors `pixfer-core/src/tx_screen.rs`.

// A transmitter-side screen renderer.
export class TxScreen {
  // hal: an object with { open(), close(), render(blocks), supportsMulticolor() }.
  constructor(hal) {
    this.hal = hal;
  }

  // Open the display and render a block grid.
  render(blocks) {
    this.hal.open();
    return this.hal.render(blocks);
  }

  // Whether the display reports multicolor support (8-color palette).
  supportsMulticolor() {
    return this.hal.supportsMulticolor();
  }

  // Close the display.
  close() {
    this.hal.close();
  }
}