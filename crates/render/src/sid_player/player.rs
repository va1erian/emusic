//! Player: drives the 6510 and renders SID audio.
//!
//! Adapted from the `sidera` crate's `sidplay` example (MIT, © Sébastien
//! Béchet).

use mos6502::cpu::CPU;
use mos6502::instruction::Nmos6502;
use mos6502::memory::Bus;
use mos6502::registers::StackPointer;
use sidera::{ChipModel, Sid};

use super::bus::C64;
use super::psid::Psid;

type Cpu = CPU<C64, Nmos6502>;

/// Upper bound on the multispeed factor, so a garbage CIA latch cannot turn
/// into hundreds of play calls per frame.
const MAX_SPEED: u32 = 16;

/// One PSID tune: a 6510 with the SID trapped at `$D400`.
pub(crate) struct Player {
    pub(crate) cpu: Cpu,
    play_addr: u16,
    sample_rate: u32,
    refresh: u32,
    /// Play calls per video frame (1 for a normal VBI tune).
    speed: u32,
    /// Bresenham accumulator so samples-per-frame averages exactly.
    frac: u64,
}

impl Player {
    /// Loads `psid`, runs its `init` for `song0`, and prepares playback.
    pub(crate) fn new(
        psid: &Psid,
        song0: u8,
        model: ChipModel,
        sample_rate: u32,
        refresh: u32,
    ) -> Player {
        let mut sid = Sid::new(sample_rate);
        sid.set_model(model);
        let mut cpu = CPU::new(C64::new(sid), Nmos6502);

        let base = psid.load_addr;
        for (i, &b) in psid.data.iter().enumerate() {
            let a = base.wrapping_add(i as u16);
            cpu.memory.ram[a as usize] = b;
        }

        cpu.registers.stack_pointer = StackPointer(0xff);
        cpu.registers.accumulator = song0;
        cpu.registers.index_x = 0;
        cpu.registers.index_y = 0;
        let mut player = Player {
            cpu,
            play_addr: psid.play_addr,
            sample_rate,
            refresh,
            speed: 1,
            frac: 0,
        };
        player.call(psid.init_addr);

        // PSID play==0 tunes install their own IRQ handler and expect to be
        // entered as an IRQ. KERNAL `$0314` handlers often end with `JMP $EA31`,
        // which pulls Y/X/A and RTIs; plant that stub (this bus has no ROM) and
        // a bare RTI at $EA81 for the no-register-restore path.
        for (off, byte) in [0x68u8, 0xA8, 0x68, 0xAA, 0x68, 0x40]
            .into_iter()
            .enumerate()
        {
            player.cpu.memory.ram[0xEA31 + off] = byte;
        }
        player.cpu.memory.ram[0xEA81] = 0x40;

        let ntsc = psid.prefer_ntsc == Some(true);
        let raw = Self::multispeed_factor(&player.cpu, psid.is_cia_timed(song0), ntsc, refresh);
        player.speed = raw.min(MAX_SPEED);
        player
    }

    /// Play calls per frame, `round(frame_cycles / CIA latch)` (or 1 for a
    /// normal VBI tune). The latch is read back from RAM after init.
    fn multispeed_factor(cpu: &Cpu, cia_timed: bool, ntsc: bool, refresh: u32) -> u32 {
        if !cia_timed || refresh == 0 {
            return 1;
        }
        let latch = cpu.memory.ram[0xDC04] as u32 | ((cpu.memory.ram[0xDC05] as u32) << 8);
        if latch == 0 {
            return 1;
        }
        let clock = if ntsc { 1_022_727 } else { 985_248u32 };
        let frame_cycles = clock / refresh;
        ((frame_cycles + latch / 2) / latch).max(1)
    }

    /// Invoke the tune once: its `play`, or the installed IRQ handler when the
    /// PSID play address is 0.
    fn step_play(&mut self) {
        if self.play_addr != 0 {
            self.call(self.play_addr);
        } else {
            self.call_irq();
        }
    }

    /// Drive the tune's installed interrupt handler once (PSID `play == 0`).
    fn call_irq(&mut self) -> bool {
        let rb = |m: &mut Cpu, a: u16| m.memory.ram[a as usize];
        let kernal = (rb(&mut self.cpu, 0x0314) as u16) | ((rb(&mut self.cpu, 0x0315) as u16) << 8);
        let hw = (rb(&mut self.cpu, 0xfffe) as u16) | ((rb(&mut self.cpu, 0xffff) as u16) << 8);
        let (vector, push_axy) = if kernal != 0 {
            (kernal, true)
        } else if hw != 0 {
            (hw, false)
        } else {
            return false;
        };

        let sp0 = self.cpu.registers.stack_pointer.0;
        let (a, x, y) = (
            self.cpu.registers.accumulator,
            self.cpu.registers.index_x,
            self.cpu.registers.index_y,
        );
        let p_byte = (self.cpu.registers.status.bits() & !0x10) | 0x24;
        let push = |cpu: &mut Cpu, v: u8| {
            let sp = cpu.registers.stack_pointer.0;
            cpu.memory.set_byte(0x0100 + sp as u16, v);
            cpu.registers.stack_pointer = StackPointer(sp.wrapping_sub(1));
        };
        push(&mut self.cpu, 0x00);
        push(&mut self.cpu, 0x00);
        push(&mut self.cpu, p_byte);
        if push_axy {
            push(&mut self.cpu, a);
            push(&mut self.cpu, x);
            push(&mut self.cpu, y);
        }
        self.cpu.registers.program_counter = vector;

        let mut guard: u32 = 0;
        while self.cpu.registers.stack_pointer.0 != sp0 {
            if !self.cpu.single_step() {
                break;
            }
            guard += 1;
            if guard > 5_000_000 {
                break;
            }
        }
        true
    }

    /// Run a subroutine to completion via a 2-byte sentinel return, bounded by a
    /// runaway guard.
    fn call(&mut self, addr: u16) {
        let sp0 = self.cpu.registers.stack_pointer.0;
        let mut sp = sp0;
        self.cpu.memory.set_byte(0x0100 + sp as u16, 0x00);
        sp = sp.wrapping_sub(1);
        self.cpu.memory.set_byte(0x0100 + sp as u16, 0x00);
        sp = sp.wrapping_sub(1);
        self.cpu.registers.stack_pointer = StackPointer(sp);
        self.cpu.registers.program_counter = addr;

        let mut guard: u32 = 0;
        while self.cpu.registers.stack_pointer.0 != sp0 {
            if !self.cpu.single_step() {
                break;
            }
            guard += 1;
            if guard > 5_000_000 {
                break;
            }
        }
    }

    /// Render one video frame's samples, interleaved between the play calls so
    /// sub-frame register writes land at roughly the right time.
    pub(crate) fn render_frame(&mut self) -> Vec<i16> {
        self.frac += u64::from(self.sample_rate);
        let n = (self.frac / u64::from(self.refresh)) as usize;
        self.frac %= u64::from(self.refresh);

        let mut out = Vec::with_capacity(n);
        let speed = u64::from(self.speed);
        for k in 0..speed {
            self.step_play();
            let lo = (n as u64 * k / speed) as usize;
            let hi = (n as u64 * (k + 1) / speed) as usize;
            for _ in lo..hi {
                out.push(self.cpu.memory.sid.clock());
            }
        }
        out
    }
}
