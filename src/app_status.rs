use std::collections::HashMap;
use std::iter::Iterator;

use strum::EnumString;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, EnumString)]
pub enum AppJob {
    DiscoveringPhotos,
    GeneratingThumbnails,
    LoadingFonts,
}

impl AppJob {
    fn title(&self) -> &'static str {
        match *self {
            AppJob::DiscoveringPhotos => "Discovering Photos",
            AppJob::GeneratingThumbnails => "Generating Thumbnails",
            AppJob::LoadingFonts => "Loading Fonts",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppJobStatus {
    Indefinite,
    Finite { complete: usize, total: usize },
}

#[derive(Debug, Clone)]
pub struct AppStatus {
    active: HashMap<AppJob, AppJobStatus>,
}

#[derive(Debug, Clone)]
pub struct AppStatusIterItem<'a> {
    pub job: &'a AppJob,
    pub status: &'a AppJobStatus,
}

impl<'a> AppStatusIterItem<'a> {
    pub fn description(&self) -> String {
        match self.status {
            AppJobStatus::Indefinite => format!("{}", self.job.title()),
            AppJobStatus::Finite { complete, total } => {
                format!("{} ({}/{})", self.job.title(), complete, total)
            }
        }
    }
}

impl AppStatus {
    pub fn new() -> Self {
        AppStatus {
            active: HashMap::new(),
        }
    }

    pub fn start_finite(&mut self, job: AppJob, total: usize) {
        self.active
            .insert(job, AppJobStatus::Finite { complete: 0, total });
    }

    pub fn start_indefinitie(&mut self, job: AppJob) {
        self.active.insert(job, AppJobStatus::Indefinite);
    }

    pub fn update(&mut self, job: AppJob, status: AppJobStatus) {
        self.active.insert(job, status);
    }

    pub fn complete(&mut self, job: AppJob) {
        self.active.remove(&job);
    }

    pub fn increment(&mut self, job: AppJob) {
        if let Some(item) = self.active.get_mut(&job) {
            match item {
                AppJobStatus::Indefinite => return,
                AppJobStatus::Finite { complete, total } => {
                    *item = AppJobStatus::Finite {
                        complete: *complete + 1,
                        total: *total,
                    }
                }
            }
        }
    }

    pub fn increment_and_complete_if_done(&mut self, job: AppJob) {
        self.increment(job);
        if self.is_complete(job) {
            self.complete(job);
        }
    }

    pub fn is_complete(&self, job: AppJob) -> bool {
        if let Some(job) = self.active.get(&job) {
            return match job {
                AppJobStatus::Indefinite => false,
                AppJobStatus::Finite { complete, total } => complete >= total,
            };
        }
        true
    }

    pub fn iter(&self) -> impl Iterator<Item = AppStatusIterItem<'_>> {
        self.active
            .iter()
            .map(|(job, status)| AppStatusIterItem { job, status })
    }
}
