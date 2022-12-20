use log::{Level, LevelFilter, Metadata, Record};

static LOGGER: HastyLogger = HastyLogger;

pub struct HastyLogger;

impl log::Log for HastyLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            // ref: https://doc.rust-lang.org/std/fmt/#fillalignment
            println!("{} - {}", format!("{:<12}", record.target()), record.args());
        }
    }

    fn flush(&self) {}
}

pub fn init() {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Info))
        .unwrap();
}
