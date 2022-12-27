use log::{Level, LevelFilter, Metadata, Record};

static LOGGER: HastyLogger = HastyLogger;

pub struct HastyLogger;

impl log::Log for HastyLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let prefix = match record.target() {
                "hasty" => String::from("[hasty] "),
                _ => {
                    // ref: https://doc.rust-lang.org/std/fmt/#fillalignment
                    format!("{:<12} - ", record.target())
                }
            };

            println!("{}{}", prefix, record.args());
        }
    }

    fn flush(&self) {}
}

pub fn init() {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Info))
        .unwrap();
}
