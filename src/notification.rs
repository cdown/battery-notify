use log::{error, trace};
use notify_rust::{Notification, NotificationHandle, Urgency};

pub struct SingleNotification {
    hnd: Option<NotificationHandle>,
    summary: String,
}

impl SingleNotification {
    pub const fn new() -> Self {
        Self {
            hnd: None,
            summary: String::new(),
        }
    }

    pub fn show(&mut self, summary: String, urgency: Urgency) {
        if self.summary != summary {
            self.close();
            self.summary = summary;
            trace!("Creating notification for {}", self.summary);
            self.hnd = Notification::new()
                .summary(&self.summary)
                .urgency(urgency)
                .show()
                .map_err(|err| error!("error showing notification: {err}"))
                .ok();
        }
    }

    pub fn close(&mut self) {
        if let Some(hnd) = self.hnd.take() {
            trace!("Closing notification for {}", self.summary);
            hnd.close();
        }
    }
}

impl Drop for SingleNotification {
    fn drop(&mut self) {
        self.close();
    }
}
