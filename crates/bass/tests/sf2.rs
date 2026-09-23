//! Test helper: builds a minimal but valid SoundFont 2 file (one preset, one
//! instrument, one looping 441 Hz sine sample) so MIDI tests can check that
//! BASSMIDI really renders sound without needing a real soundfont on disk.

fn chunk(id: &[u8; 4], mut data: Vec<u8>) -> Vec<u8> {
    if data.len() % 2 == 1 {
        data.push(0);
    }
    let mut out = id.to_vec();
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend(data);
    out
}

fn list(kind: &[u8; 4], body: Vec<u8>) -> Vec<u8> {
    let mut data = kind.to_vec();
    data.extend(body);
    chunk(b"LIST", data)
}

fn name(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.resize(20, 0);
    bytes
}

fn words(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Returns the bytes of a one-preset soundfont playing a looped sine wave.
pub fn sine_soundfont() -> Vec<u8> {
    const SAMPLES: u32 = 100;
    let mut smpl: Vec<u8> = (0..SAMPLES)
        .flat_map(|i| {
            let phase = 2.0 * std::f64::consts::PI * f64::from(i) / f64::from(SAMPLES);
            ((20000.0 * phase.sin()) as i16).to_le_bytes()
        })
        .collect();
    smpl.extend(std::iter::repeat_n(0u8, 46 * 2));

    let info = list(
        b"INFO",
        [
            chunk(b"ifil", words(&[2, 1])),
            chunk(b"isng", b"EMU8000\0".to_vec()),
            chunk(b"INAM", b"TestSine\0".to_vec()),
        ]
        .concat(),
    );
    let sdta = list(b"sdta", chunk(b"smpl", smpl));

    let record = |label: &str, tail: Vec<u8>| [name(label), tail].concat();
    let phdr = [
        record("Sine", [words(&[0, 0, 0]), vec![0; 12]].concat()),
        record("EOP", [words(&[0, 0, 1]), vec![0; 12]].concat()),
    ]
    .concat();
    let pgen = words(&[41, 0, 0, 0]); // instrument 0, then the terminal record
    let inst = [record("SineInst", words(&[0])), record("EOI", words(&[1]))].concat();
    // Sample modes = loop, then the mandatory final sample id generator.
    let igen = words(&[54, 1, 53, 0, 0, 0]);
    let mut sample_tail = Vec::new();
    for value in [0u32, SAMPLES, 0, SAMPLES, 44100] {
        sample_tail.extend_from_slice(&value.to_le_bytes());
    }
    sample_tail.extend_from_slice(&[45, 0]); // root key, correction
    sample_tail.extend(words(&[0, 1])); // sample link, mono type
    let shdr = [record("sine", sample_tail), record("EOS", vec![0; 26])].concat();

    let pdta = list(
        b"pdta",
        [
            chunk(b"phdr", phdr),
            chunk(b"pbag", words(&[0, 0, 1, 0])),
            chunk(b"pmod", vec![0; 10]),
            chunk(b"pgen", pgen),
            chunk(b"inst", inst),
            chunk(b"ibag", words(&[0, 0, 2, 0])),
            chunk(b"imod", vec![0; 10]),
            chunk(b"igen", igen),
            chunk(b"shdr", shdr),
        ]
        .concat(),
    );

    let body = [b"sfbk".to_vec(), info, sdta, pdta].concat();
    chunk(b"RIFF", body)
}
