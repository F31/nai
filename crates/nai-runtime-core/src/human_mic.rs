//! Private human-route leases. Status/health cannot activate a microphone.
#[derive(Default)]
pub struct Gate {
    pub seq: u64,
    pub challenge: u64,
    issued: u64,
    pub deadline: u64,
    pub session: String,
    pub prepared: bool,
    pub active: bool,
    until: u64,
    used: Vec<String>,
}
impl Gate {
    pub fn issue(&mut self, now: u64) {
        self.challenge = self.challenge.saturating_add(1);
        self.issued = now;
    }
    pub fn live(&self, now: u64) -> bool {
        self.active && now < self.deadline && now < self.until
    }
    pub fn stop(&mut self) {
        self.active = false;
        self.prepared = false;
        self.deadline = 0;
    }
    pub fn command(
        &mut self,
        x: &nai_contracts::workbench::MicControl,
        now: u64,
    ) -> Result<(), &'static str> {
        if x.seq != self.seq + 1 {
            return Err("SEQUENCE_REJECTED");
        }
        self.seq = x.seq;
        if x.action == 0 {
            return Ok(());
        }
        if x.action == 3 {
            self.stop();
            return Ok(());
        }
        if x.challenge != self.challenge || now < self.issued || now - self.issued >= 250 {
            return Err("CHALLENGE_EXPIRED");
        }
        if x.action == 1 {
            if self.active
                || self.prepared
                || !x.acknowledge_route
                || x.input_id == 0
                || x.output_id == 0
                || self.used.len() >= 32
                || self.used.contains(&x.session_id)
            {
                return Err("START_REJECTED");
            }
            if crate::foundation::uuid(&x.session_id).is_err() {
                return Err("SESSION_REJECTED");
            }
            self.used.push(x.session_id.clone());
            self.session = x.session_id.clone();
            self.until = now + 14400000;
            self.prepared = true;
        } else {
            if x.session_id != self.session {
                return Err("SESSION_REJECTED");
            }
            match x.action {
                4 if self.prepared && !self.active => {
                    self.active = true;
                }
                2 if self.live(now) => (),
                _ => return Err("LEASE_EXPIRED"),
            }
            self.deadline = (self.issued + 500).min(self.until);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use nai_contracts::workbench::MicControl;
    fn c(action: u32, seq: u64, ch: u64) -> MicControl {
        MicControl {
            action,
            seq,
            challenge: ch,
            session_id: "00000000-0000-4000-8000-000000000001".into(),
            input_id: 1,
            output_id: 2,
            acknowledge_route: true,
            ..Default::default()
        }
    }
    #[test]
    fn microphone_needs_prepare_and_fresh_activation_and_never_renews_from_health() {
        let mut g = Gate::default();
        g.issue(0);
        g.command(&c(1, 1, 1), 1).unwrap();
        assert!(!g.live(1));
        g.issue(100);
        g.command(&c(4, 2, 2), 110).unwrap();
        assert!(g.live(599));
        assert!(!g.live(600));
        g.issue(600);
        assert!(g.command(&c(2, 3, 3), 600).is_err());
        g.stop();
        g.issue(601);
        assert!(g.command(&c(1, 4, 4), 601).is_err());
    }
    #[test]
    fn stale_challenge_wrong_session_and_replay_do_not_create_authority() {
        let mut g = Gate::default();
        g.issue(0);
        assert!(g.command(&c(1, 1, 1), 250).is_err());
        g.issue(251);
        g.command(&c(1, 2, 2), 251).unwrap();
        g.issue(252);
        g.command(&c(4, 3, 3), 252).unwrap();
        let until = g.deadline;
        g.issue(300);
        g.command(&c(0, 4, 0), 300).unwrap();
        assert_eq!(g.deadline, until);
        g.issue(301);
        let mut bad = c(2, 5, 5);
        bad.session_id = "x".into();
        assert!(g.command(&bad, 301).is_err());
        assert_eq!(g.deadline, until);
        g.command(&c(3, 6, 0), 302).unwrap();
        assert!(!g.live(302));
    }
}
