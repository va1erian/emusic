//! C64 bus: 64 KB RAM with the SID mapped at `$D400-$D7FF`.
//!
//! Adapted from the `sidera` crate's `sidplay` example (MIT, © Sébastien
//! Béchet), trimmed to just what playback needs.

use mos6502::memory::Bus;
use sidera::Sid;
use sidera::registers::{ENV3, OSC3};

pub(crate) struct C64 {
    pub(crate) ram: Box<[u8; 0x10000]>,
    pub(crate) sid: Sid,
}

impl C64 {
    pub(crate) fn new(sid: Sid) -> C64 {
        // Work RAM is zero-filled: feeding tunes a DRAM pattern makes some read
        // uninitialised bytes as pointers and halt; see the upstream notes.
        let mut ram = Box::new([0u8; 0x10000]);
        ram[0x00] = 0x2f; // data-direction register
        ram[0x01] = 0x37; // default banking (RAM + I/O visible)
        C64 { ram, sid }
    }
}

impl Bus for C64 {
    fn get_byte(&mut self, addr: u16) -> u8 {
        if (0xd400..=0xd7ff).contains(&addr) {
            // Only OSC3/ENV3 read back; other registers float.
            let reg = ((addr - 0xd400) & 0x1f) as u8;
            return match reg {
                OSC3 | ENV3 => self.sid.read_register(reg),
                _ => 0xff,
            };
        }
        self.ram[addr as usize]
    }

    fn set_byte(&mut self, addr: u16, value: u8) {
        if (0xd400..=0xd7ff).contains(&addr) {
            let reg = ((addr - 0xd400) & 0x1f) as u8;
            self.sid.write_register(reg, value);
            return;
        }
        self.ram[addr as usize] = value;
    }
}
