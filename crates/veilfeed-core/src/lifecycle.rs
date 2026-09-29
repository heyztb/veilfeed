use crate::AppService;
use tokio::{
    sync::watch,
    task::JoinHandle,
    time::{Duration, MissedTickBehavior},
};

pub struct BackgroundTasks {
    stop: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}
impl BackgroundTasks {
    // Feed refresh remains scheduled by the Tauri frontend.
    pub fn start(service: AppService) -> Self {
        let (stop, mut stopped) = watch::channel(false);
        let mut tasks = vec![];
        tasks.push(tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(3600));
            timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    biased;
                    _ = stopped.changed() => break,
                    _ = service.blocklist_wake.notified() => refresh_blocklists(&service).await,
                    _ = timer.tick() => refresh_blocklists(&service).await,
                }
            }
        }));
        Self { stop, tasks }
    }
    pub async fn shutdown(mut self) {
        let _ = self.stop.send(true);
        for task in self.tasks.drain(..) {
            task.abort();
            let _ = task.await;
        }
    }
}
impl Drop for BackgroundTasks {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        for task in &self.tasks {
            task.abort();
        }
    }
}

// `AdFilter::refresh` is a no-op while automatic updates are disabled.
async fn refresh_blocklists(service: &AppService) {
    match service.db.proxy(None).await {
        Ok(profile) => {
            if let Err(e) = service
                .ad_filter
                .refresh(&service.network, profile.as_ref())
                .await
            {
                eprintln!("blocker-list update failed; retaining current rules: {e}");
            }
        }
        Err(e) => eprintln!("blocker-list route unavailable; retaining current rules: {e}"),
    }
}
