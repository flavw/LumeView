//! Tracks disk writes independently of other background work.
#[derive(Default)]
pub struct SaveGuard {
    pending: usize,
    close_after: bool,
}
impl SaveGuard {
    pub fn begin(&mut self) {self.pending+=1;}
    pub fn active(&self) -> bool {self.pending>0}
    pub fn waiting(&self) -> bool {self.close_after}
    pub fn wait(&mut self) {self.close_after=self.active();}
    pub fn cancel_wait(&mut self) {self.close_after=false;}
    /// Close only after every write succeeds; any failure cancels queued closing.
    pub fn finish(&mut self,success: bool) -> bool {
        if self.pending==0 {return false;}
        self.pending-=1;
        if !success {self.close_after=false;}
        if self.pending==0 {return std::mem::take(&mut self.close_after);}
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn waiting_closes_only_after_all_writes_complete() {
        let mut saves=SaveGuard::default();saves.begin();saves.begin();saves.wait();
        assert!(!saves.finish(true));assert!(saves.active());
        assert!(saves.finish(true));assert!(!saves.active());assert!(!saves.waiting());
        assert!(!saves.finish(true));
    }
    #[test]
    fn failure_or_cancelling_the_close_keeps_the_app_open() {
        let mut saves=SaveGuard::default();saves.begin();saves.begin();saves.wait();
        assert!(!saves.finish(false));assert!(!saves.waiting());
        assert!(!saves.finish(true));
        saves.begin();saves.wait();saves.cancel_wait();assert!(!saves.finish(true));
        saves.begin();assert!(!saves.finish(true));
    }
}
