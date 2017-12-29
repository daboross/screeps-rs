macro_rules! yield_from {
    ($g:expr) => ({
        let mut gen = $g;
        loop {
            let state = gen.resume();

            match state {
                GeneratorState::Yielded(v) => yield v,
                GeneratorState::Complete(r) => break r,
            }
        }
    })
}
