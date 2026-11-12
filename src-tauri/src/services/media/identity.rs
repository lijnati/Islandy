//! Mapping native session objects to stable Islandy session ids (pure).
//!
//! `GetSessions()` returns a fresh list on every call. Sessions are matched to
//! the ones already tracked by COM identity first; if an object's identity
//! changed, a unique match on the source app id is accepted. Everything else
//! is a new or a removed session. Source app ids alone are never trusted as
//! session identity, because one app can expose several sessions.

// Parts of this module are only reached from the Windows backend.
#![cfg_attr(not(windows), allow(dead_code))]

use super::models::SessionId;

#[derive(Debug, Clone)]
pub struct Known<'a, K> {
    pub id: SessionId,
    pub identity: K,
    pub app_id: &'a str,
}

#[derive(Debug, Clone)]
pub struct Incoming<'a, K> {
    pub identity: K,
    pub app_id: &'a str,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Reconciliation {
    /// `(incoming index, existing id)`.
    pub matched: Vec<(usize, SessionId)>,
    /// Incoming indices that are new sessions.
    pub added: Vec<usize>,
    pub removed: Vec<SessionId>,
}

pub fn reconcile<K: PartialEq>(
    known: &[Known<'_, K>],
    incoming: &[Incoming<'_, K>],
) -> Reconciliation {
    let mut known_used = vec![false; known.len()];
    let mut incoming_match: Vec<Option<SessionId>> = vec![None; incoming.len()];

    // 1. COM identity.
    for (i, item) in incoming.iter().enumerate() {
        if let Some(k) = known
            .iter()
            .enumerate()
            .position(|(k, known)| !known_used[k] && known.identity == item.identity)
        {
            known_used[k] = true;
            incoming_match[i] = Some(known[k].id);
        }
    }

    // 2. Unique app id among the leftovers on both sides.
    for i in 0..incoming.len() {
        if incoming_match[i].is_some() {
            continue;
        }
        let app = incoming[i].app_id;
        let incoming_same = (0..incoming.len())
            .filter(|&j| incoming_match[j].is_none() && incoming[j].app_id == app)
            .count();
        let known_same: Vec<usize> = (0..known.len())
            .filter(|&k| !known_used[k] && known[k].app_id == app)
            .collect();
        if incoming_same == 1 && known_same.len() == 1 {
            known_used[known_same[0]] = true;
            incoming_match[i] = Some(known[known_same[0]].id);
        }
    }

    let mut result = Reconciliation::default();
    for (i, matched) in incoming_match.into_iter().enumerate() {
        match matched {
            Some(id) => result.matched.push((i, id)),
            None => result.added.push(i),
        }
    }
    result.removed = known
        .iter()
        .zip(known_used)
        .filter(|(_, used)| !used)
        .map(|(k, _)| k.id)
        .collect();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(id: u64, identity: u32, app: &str) -> Known<'_, u32> {
        Known {
            id: SessionId(id),
            identity,
            app_id: app,
        }
    }
    fn inc(identity: u32, app: &str) -> Incoming<'_, u32> {
        Incoming {
            identity,
            app_id: app,
        }
    }

    #[test]
    fn matches_by_identity_and_detects_additions_and_removals() {
        let k = [known(1, 10, "A"), known(2, 20, "B")];
        let i = [inc(20, "B"), inc(30, "C")];
        let r = reconcile(&k, &i);
        assert_eq!(r.matched, [(0, SessionId(2))]);
        assert_eq!(r.added, [1]);
        assert_eq!(r.removed, [SessionId(1)]);
    }

    #[test]
    fn falls_back_to_a_unique_app_id_when_identity_changed() {
        let k = [known(1, 10, "Player.exe")];
        let i = [inc(11, "Player.exe")];
        assert_eq!(reconcile(&k, &i).matched, [(0, SessionId(1))]);
    }

    #[test]
    fn never_guesses_between_sessions_sharing_an_app_id() {
        // Two browser tabs: identity changed for both; ambiguous → new ids.
        let k = [known(1, 10, "Browser"), known(2, 20, "Browser")];
        let i = [inc(11, "Browser"), inc(21, "Browser")];
        let r = reconcile(&k, &i);
        assert!(r.matched.is_empty());
        assert_eq!(r.added, [0, 1]);
        assert_eq!(r.removed, [SessionId(1), SessionId(2)]);

        // One tab closed, the other kept its identity.
        let i = [inc(20, "Browser")];
        let r = reconcile(&k, &i);
        assert_eq!(r.matched, [(0, SessionId(2))]);
        assert_eq!(r.removed, [SessionId(1)]);
    }

    #[test]
    fn handles_empty_lists() {
        let r = reconcile::<u32>(&[], &[]);
        assert_eq!(r, Reconciliation::default());
        let k = [known(1, 10, "A")];
        assert_eq!(reconcile(&k, &[]).removed, [SessionId(1)]);
    }
}
