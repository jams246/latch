use super::*;

#[derive(Default)]
struct Events {
    text: String,
    commands: Vec<Vec<Vec<u8>>>,
}

impl Perform for Events {
    fn print(&mut self, character: char) {
        self.text.push(character);
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _: bool) {
        self.commands.push(params.iter().map(|param| param.to_vec()).collect());
    }
}

#[test]
fn oversized_osc_stays_bounded_and_recovers_after_terminator() {
    for terminator in [b"\x07".as_slice(), b"\x1b\\".as_slice()] {
        let mut parser = Parser::new();
        let mut events = Events::default();
        parser.advance(&mut events, b"\x1b]0;");
        for _ in 0..32 {
            parser.advance(&mut events, &[b'x'; 1024]);
        }
        assert_eq!(parser.osc_raw.len(), MAX_OSC_RAW_STD);
        parser.advance(&mut events, terminator);
        assert!(events.commands.is_empty());
        parser.advance(&mut events, b"ok\x1b]0;title\x07");
        assert_eq!(events.text, "ok");
        assert_eq!(events.commands, vec![vec![b"0".to_vec(), b"title".to_vec()]]);
    }
}

#[test]
fn osc_at_limit_preserves_complete_command() {
    let mut parser = Parser::new();
    let mut events = Events::default();
    parser.advance(&mut events, b"\x1b]0;");
    parser.advance(&mut events, &vec![b'x'; MAX_OSC_RAW_STD - 1]);
    parser.advance(&mut events, b"\x07");
    assert_eq!(events.commands[0][1].len(), MAX_OSC_RAW_STD - 1);
}
