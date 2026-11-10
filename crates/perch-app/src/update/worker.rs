//! The thread the check and the download run on, and the status it publishes.
//!
//! One thread, one job at a time, a status read by the frame with no lock. A check is posted when
//! the worker starts, so startup costs a spawn and never a request; a press posts a stage job, so
//! a thirty-second download costs the window nothing either. What the frame reads is whatever
//! this thread published last, which is why no frame can be waiting on a socket.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};

use arc_swap::ArcSwap;
use perch_update::{Manifest, Transport, Version, check, stage};

use super::{Status, UPDATES};

/// What the thread has been asked to do.
enum Job {
    /// Read the manifest and decide whether it is an update.
    Check,
    /// Fetch and verify the archive of the release the last check offered.
    Stage,
}

/// The update thread and the status it last published.
pub struct Worker {
    latest: Arc<ArcSwap<Status>>,
    jobs: Sender<Job>,
    /// Whether a job is being done. Read by the frame to keep drawing until the status lands.
    working: Arc<AtomicBool>,
}

impl Worker {
    /// Starts the thread with the check already posted, answering at once.
    ///
    /// The transport is shared with the caller, which is how a test observes the requests after
    /// the fact: nothing here is copied and no request is made on this thread.
    pub fn start(url: String, dir: PathBuf, transport: Arc<dyn Transport>) -> Worker {
        let latest = Arc::new(ArcSwap::from_pointee(Status::Checking));
        let working = Arc::new(AtomicBool::new(true));
        let (jobs, queued) = mpsc::channel();
        let published = Arc::clone(&latest);
        let busy = Arc::clone(&working);
        std::thread::Builder::new()
            .name("perch-update".to_string())
            .spawn(move || run(&url, &dir, &*transport, &published, &busy, &queued))
            .expect("spawn the update thread");
        let _ = jobs.send(Job::Check);
        Worker {
            latest,
            jobs,
            working,
        }
    }

    /// The status this thread last published.
    pub fn latest(&self) -> Arc<Status> {
        self.latest.load_full()
    }

    /// Asks for the offered archive to be downloaded and staged, without waiting.
    pub fn stage(&self) {
        let _ = self.jobs.send(Job::Stage);
    }

    /// Whether a job is in flight, which is what keeps frames coming until its status is read.
    pub fn busy(&self) -> bool {
        self.working.load(Ordering::Relaxed) || matches!(*self.latest(), Status::Checking)
    }
}

/// The thread's body: one job, one publish, until the app drops the sender.
fn run(
    url: &str,
    dir: &Path,
    transport: &dyn Transport,
    latest: &ArcSwap<Status>,
    working: &AtomicBool,
    jobs: &Receiver<Job>,
) {
    // The manifest the last check read, so a press stages the release the line named rather than
    // whatever the server has moved to since.
    let mut offer: Option<Manifest> = None;
    for job in jobs {
        working.store(true, Ordering::Relaxed);
        let status = match job {
            Job::Check => checked(url, transport, &mut offer),
            Job::Stage => staged(dir, transport, offer.as_ref()),
        };
        latest.store(Arc::new(status));
        working.store(false, Ordering::Relaxed);
    }
}

/// Reads the manifest and answers what it means for this build.
fn checked(url: &str, transport: &dyn Transport, offer: &mut Option<Manifest>) -> Status {
    let text = match manifest_text(url, transport) {
        Ok(text) => text,
        Err(why) => return Status::Failed(why),
    };
    match check(env!("CARGO_PKG_VERSION"), &text) {
        Ok(None) => Status::Current,
        Ok(Some(update)) => {
            // `check` parsed the same document, so this cannot fail; if it somehow did, the press
            // would refuse to stage and say so rather than download something unvouched for.
            *offer = Manifest::parse(&text).ok();
            Status::Offer {
                version: update.version,
                notes: update.notes,
            }
        }
        Err(error) => Status::Failed(error.to_string()),
    }
}

/// Downloads the archive of `offer` into the updates directory and verifies its checksum.
fn staged(dir: &Path, transport: &dyn Transport, offer: Option<&Manifest>) -> Status {
    let Some(manifest) = offer else {
        return Status::Failed("there is no update on offer, so nothing was staged".to_string());
    };
    let dest = destination(dir, &manifest.version);
    match stage(transport, manifest, &dest) {
        Ok(()) => Status::Staged {
            version: manifest.version.clone(),
            path: dest,
        },
        Err(error) => Status::Failed(error.to_string()),
    }
}

/// The manifest's text, with a failure that names the URL rather than the socket.
fn manifest_text(url: &str, transport: &dyn Transport) -> Result<String, String> {
    let bytes = transport.get(url).map_err(|error| error.to_string())?;
    String::from_utf8(bytes).map_err(|_| format!("the manifest at {url} is not UTF-8"))
}

/// Where a staged archive lands: `<data>/updates/perch-<version>.zip`.
fn destination(dir: &Path, version: &Version) -> PathBuf {
    dir.join(UPDATES).join(format!("perch-{version}.zip"))
}
