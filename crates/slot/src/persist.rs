use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};

use slot_store::{atomic_write, read_slot_state, write_slot_state, Core, Platform, StateRing};

pub trait Snapshot {
    fn state(&self) -> Option<Vec<u8>>;
    fn save_ram(&self) -> Option<Vec<u8>>;
    fn thumb(&self) -> Option<Vec<u8>>;
    fn load(&self, state: Vec<u8>);

    fn resume_trusted(&self) -> bool {
        true
    }

    fn save_ram_trusted(&self) -> bool {
        true
    }
}

pub fn flush(
    root: &Path,
    platform: Platform,
    core: Core,
    stem: &str,
    state: Option<&[u8]>,
    sav: Option<&[u8]>,
) -> std::io::Result<()> {
    settle();
    write_now(root, platform, core, stem, state, sav)
}

fn write_now(
    root: &Path,
    platform: Platform,
    core: Core,
    stem: &str,
    state: Option<&[u8]>,
    sav: Option<&[u8]>,
) -> std::io::Result<()> {
    if let Some(state) = state {
        StateRing::new(root, platform, core, stem).write_resume(state)?;
    }
    if let Some(sav) = sav {
        write_sav_now(root, platform, stem, sav)?;
    }
    Ok(())
}

pub fn eject(
    root: &Path,
    platform: Platform,
    core: Core,
    stem: &str,
    state: Option<&[u8]>,
    sav: Option<&[u8]>,
) -> std::io::Result<()> {
    flush(root, platform, core, stem, state, sav)?;
    let mut slot = read_slot_state(root);
    slot.cart = None;
    slot.cart_platform = None;
    write_slot_state(root, &slot)
}

pub fn flush_later(
    root: &Path,
    platform: Platform,
    core: Core,
    stem: &str,
    state: Option<Vec<u8>>,
    sav: Option<Vec<u8>>,
) {
    let job = Job {
        root: root.to_path_buf(),
        platform,
        core,
        stem: stem.to_owned(),
        state,
        sav,
    };
    let Some(w) = writer() else {
        return job.write();
    };
    let mut q = w.lock();
    match q.jobs.iter_mut().find(|j| j.same_cart(&job)) {
        Some(old) => {
            old.state = job.state.or(old.state.take());
            old.sav = job.sav.or(old.sav.take());
        }
        None => q.jobs.push(job),
    }
    w.changed.notify_all();
}

pub fn settle() {
    let Some(w) = WRITER.get().copied().flatten() else {
        return;
    };
    let mut q = w.lock();
    while q.busy || !q.jobs.is_empty() {
        q = w.changed.wait(q).unwrap_or_else(|e| e.into_inner());
    }
}

struct Job {
    root: PathBuf,
    platform: Platform,
    core: Core,
    stem: String,
    state: Option<Vec<u8>>,
    sav: Option<Vec<u8>>,
}

impl Job {
    fn same_cart(&self, other: &Job) -> bool {
        self.root == other.root
            && self.platform == other.platform
            && self.core == other.core
            && self.stem == other.stem
    }

    fn write(self) {
        if let Err(e) = write_now(
            &self.root,
            self.platform,
            self.core,
            &self.stem,
            self.state.as_deref(),
            self.sav.as_deref(),
        ) {
            eprintln!("slot: flush: {e}");
        }
    }
}

#[derive(Default)]
struct Queue {
    jobs: Vec<Job>,
    busy: bool,
}

struct Writer {
    queue: Mutex<Queue>,
    changed: Condvar,
}

impl Writer {
    fn lock(&self) -> MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn run(&self) {
        loop {
            let job = {
                let mut q = self.lock();
                q.busy = false;
                self.changed.notify_all();
                while q.jobs.is_empty() {
                    q = self.changed.wait(q).unwrap_or_else(|e| e.into_inner());
                }
                q.busy = true;
                q.jobs.remove(0)
            };
            job.write();
        }
    }
}

static WRITER: OnceLock<Option<&'static Writer>> = OnceLock::new();

fn writer() -> Option<&'static Writer> {
    *WRITER.get_or_init(|| {
        let w: &'static Writer = Box::leak(Box::new(Writer {
            queue: Mutex::new(Queue::default()),
            changed: Condvar::new(),
        }));
        std::thread::Builder::new()
            .name("slot-flush".into())
            .spawn(|| w.run())
            .map_err(|e| eprintln!("slot: flush: no writer thread, writing inline: {e}"))
            .ok()
            .map(|_| w)
    })
}

pub fn write_sav(root: &Path, platform: Platform, stem: &str, sav: &[u8]) -> std::io::Result<bool> {
    settle();
    write_sav_now(root, platform, stem, sav)
}

fn write_sav_now(root: &Path, platform: Platform, stem: &str, sav: &[u8]) -> std::io::Result<bool> {
    let path = sav_path(root, platform, stem);
    if let Some(old) = read_sav_now(root, platform, stem) {
        if old == sav {
            return Ok(false);
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    atomic_write(&path, sav)?;
    Ok(true)
}

pub fn read_sav(root: &Path, platform: Platform, stem: &str) -> Option<Vec<u8>> {
    settle();
    read_sav_now(root, platform, stem)
}

fn read_sav_now(root: &Path, platform: Platform, stem: &str) -> Option<Vec<u8>> {
    std::fs::read(sav_path(root, platform, stem))
        .or_else(|_| {
            std::fs::read(
                crate::root::saves_dir(root)
                    .join(platform.dir_name())
                    .join(format!("{stem}.srm")),
            )
        })
        .ok()
}

pub fn read_resume(root: &Path, platform: Platform, core: Core, stem: &str) -> Option<Vec<u8>> {
    settle();
    StateRing::new(root, platform, core, stem)
        .read_resume()
        .ok()
        .flatten()
}

fn sav_path(root: &Path, platform: Platform, stem: &str) -> PathBuf {
    crate::root::saves_dir(root)
        .join(platform.dir_name())
        .join(format!("{stem}.sav"))
}
