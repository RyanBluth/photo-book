use std::{
    cell::Cell,
    sync::{Arc, Mutex},
};

use flexi_logger::writers::LogWriter;

pub struct ArcStringLog {
    log: Arc<StringLog>,
}

impl ArcStringLog {
    pub fn new(log: Arc<StringLog>) -> Self {
        Self { log }
    }
}

pub struct StringLog {
    logs: Arc<Mutex<Cell<Vec<String>>>>,
}

impl StringLog {
    pub fn new() -> Self {
        Self {
            logs: Arc::new(Mutex::new(Cell::new(Vec::new()))),
        }
    }

    pub fn for_each<F>(&self, func: F)
    where
        F: FnMut(&String),
    {
        self.logs.lock().unwrap().get_mut().iter().for_each(func);
    }
}

impl LogWriter for ArcStringLog {
    fn write(
        &self,
        now: &mut flexi_logger::DeferredNow,
        record: &log::Record,
    ) -> std::io::Result<()> {
        let line = format!(
            "[{}] {} - {}",
            record.level().as_str().to_uppercase(),
            now.now().format("%Y-%m-%d %H:%M:%S"),
            record.args()
        );
        match record.level() {
            log::Level::Error => {
                eprintln!("{}", line);
            }
            _ => {
                // println!("{}", line);
            }
        }
        self.log.logs.lock().unwrap().get_mut().push(line);
        Ok(())
    }

    fn flush(&self) -> std::io::Result<()> {
        // Write to file?
        Ok(())
    }
}
