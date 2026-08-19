//! What to do about the Windows output when the headset comes or goes.
//!
//! Only the decision lives here. Reading the default endpoint, moving it, and
//! persisting the record are `win32`'s and `settings`' work; this module is
//! handed the facts and answers with an [`Action`]. Splitting it out is what
//! makes the rule testable at all — the alternative is a function that can only
//! be exercised by powering a headset off and watching a machine's sound move.
//!
//! Two rules live here, and they answer different questions.
//!
//! **Where the sound goes while the headset is off.** The rule this turns on is
//! that **a record of where the sound came from is a debt, and a debt that
//! cannot be paid is not a reason to refuse new work**. An earlier version
//! treated the record's mere presence as "already switched", which is true
//! right up until the endpoint it names stops existing — a driver reinstall or
//! a re-enumerated dongle is enough, and this device's endpoint ids do change.
//! From that moment the record could never be cleared (the endpoint was gone)
//! and could never be replaced (its presence blocked the switch), so the
//! feature was silently dead until somebody edited the registry. Hence
//! [`Facts::debt_present`]: a debt counts only while the machine still has the
//! endpoint it names.
//!
//! **Which of the headset's own endpoints gets what, once it is back.** The
//! headset presents two: a game channel and a chat channel. Windows keeps
//! separate defaults for ordinary playback and for calls, so the two channels
//! exist precisely to be pointed at different roles — and putting the sound
//! back the way it was found cannot do that, because "the way it was found" is
//! a single endpoint. Restoring that one endpoint into all three roles is worse
//! than doing nothing: it overwrites a communications default the user had set
//! correctly, so the chat channel goes quiet and the voices arrive in the game
//! channel. Hence [`Facts::split`] and the [`Action::Split`] it produces. It is
//! off by default — a headset with two channels is not a reason to assume
//! somebody wants their calls moved.

/// Which of the three device choices something refers to.
///
/// One enum rather than three parallel settings and three picker views: all
/// three answer the same question ("which Windows endpoint?") and differ only
/// in what the answer is for. Carrying the slot means one picker, one store,
/// and one place a fourth choice would be added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// Where the sound goes while the headset is powered off.
    Fallback,
    /// The headset's game channel: ordinary playback while it is on.
    Game,
    /// The headset's chat channel: calls while it is on.
    Chat,
}

/// Something worth telling the user about, rather than logging into the void.
///
/// The wording is deliberately split. `short` goes in the settings row, whose
/// description box is one line wide and collides with the title above it when
/// it wraps; `detail` goes in a balloon, which has room to say what to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// The switch is on but no fallback device was ever picked.
    NoDeviceChosen,
    /// The chosen fallback is not plugged in, so there is nowhere to move to.
    FallbackAbsent,
    /// The endpoint the sound came from never reappeared, so it is unreachable.
    RestoreGone,
    /// Windows refused the change. The interface behind it is undocumented, so
    /// this is a thing that can simply happen.
    SwitchFailed,
    /// The split is on but the game and chat channels were not both picked.
    NoChannelsChosen,
    /// They were picked, but the endpoints they name never turned up.
    ChannelsAbsent,
}

impl Problem {
    /// One short line for the settings row. Keep these under about forty
    /// characters: the description box is sized for a single line.
    pub fn short(self) -> &'static str {
        match self {
            Problem::NoDeviceChosen => "No device chosen — pick one below.",
            Problem::FallbackAbsent => "The chosen device is not connected.",
            Problem::RestoreGone => "Could not move the sound back.",
            Problem::SwitchFailed => "Windows refused the change.",
            Problem::NoChannelsChosen => "Pick a game and a chat channel below.",
            Problem::ChannelsAbsent => "The chosen channels are not connected.",
        }
    }

    /// A stable name for the registry.
    ///
    /// The record has to survive the tray being closed, and it has to come back
    /// as *the problem it was* rather than as a sentence somebody once wrote:
    /// the panel decides which row carries it, and re-worded text must not
    /// change where a stored complaint appears. An unknown name reads as no
    /// complaint, which is what an older version's record degrades to.
    pub fn key(self) -> &'static str {
        match self {
            Problem::NoDeviceChosen => "NoDeviceChosen",
            Problem::FallbackAbsent => "FallbackAbsent",
            Problem::RestoreGone => "RestoreGone",
            Problem::SwitchFailed => "SwitchFailed",
            Problem::NoChannelsChosen => "NoChannelsChosen",
            Problem::ChannelsAbsent => "ChannelsAbsent",
        }
    }

    /// The inverse of [`Problem::key`].
    pub fn from_key(key: &str) -> Option<Problem> {
        [
            Problem::NoDeviceChosen,
            Problem::FallbackAbsent,
            Problem::RestoreGone,
            Problem::SwitchFailed,
            Problem::NoChannelsChosen,
            Problem::ChannelsAbsent,
        ]
        .into_iter()
        .find(|p| p.key() == key)
    }

    /// Whether this is a complaint about the split rather than the switch.
    ///
    /// The two are separate settings with separate rows, and either can be on
    /// while the other is off, so a complaint shown against the wrong one is a
    /// message about a feature the user may not even be using.
    pub fn is_about_split(self) -> bool {
        matches!(self, Problem::NoChannelsChosen | Problem::ChannelsAbsent)
    }

    /// The balloon text, which has room to say what happened and what to do.
    pub fn detail(self) -> &'static str {
        match self {
            Problem::NoDeviceChosen => {
                "The headset powered off, but no device has been chosen to play \
                 through. Open Settings and pick one under \"Play through\"."
            }
            Problem::FallbackAbsent => {
                "The headset powered off, but the device chosen to play through \
                 is not connected. The sound has been left where it is."
            }
            Problem::RestoreGone => {
                "The headset is back, but the device the sound came from is no \
                 longer available, so it could not be moved back. Choosing that \
                 output once in Windows will let this work again."
            }
            Problem::SwitchFailed => {
                "Windows would not change the default playback device. Setting \
                 it by hand in Sound settings still works."
            }
            Problem::NoChannelsChosen => {
                "The headset is back, but its game and chat channels have not \
                 both been chosen, so the sound was left where it is. Open \
                 Settings and pick them under \"Game channel\" and \"Chat \
                 channel\"."
            }
            Problem::ChannelsAbsent => {
                "The headset is back, but the endpoints chosen as its game and \
                 chat channels did not appear. If the device was re-installed, \
                 pick them again in Settings."
            }
        }
    }
}

/// What the tray should do about the output right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Leave everything alone.
    Nothing,
    /// Make `to` the default output and record `owing` as the way back.
    MoveAway { to: String, owing: String },
    /// Make `to` the default output and clear the record.
    MoveBack { to: String },
    /// Give ordinary playback to `game` and calls to `chat`, and clear the
    /// record: the sound has been placed deliberately, so nothing is left owed.
    Split { game: String, chat: String },
    /// The endpoint owed is not there yet. Ask again shortly rather than
    /// concluding anything: an endpoint can take a few seconds to appear after
    /// the wireless link comes up.
    Retry,
    /// It never appeared. Clear the record and say so — keeping a debt nothing
    /// can discharge is how the feature used to wedge itself.
    GiveUp,
    /// Nothing was done, and the user should be told why.
    Blocked(Problem),
}

/// How many times to look for the endpoints owed before giving up on them.
///
/// Five at [`RETRY_MS`] apart is about ten seconds, which is longer than an
/// endpoint has been observed taking to appear and short enough that a person
/// who powered their headset on is still sitting there when it resolves.
pub const RESTORE_ATTEMPTS: u8 = 5;

/// How long to wait between those looks.
pub const RETRY_MS: u32 = 2000;

/// Everything the decision depends on, gathered by the caller.
///
/// Taken as a struct rather than a dozen positional arguments because most of
/// them are `Option<&str>` and `bool`, and a transposed pair would compile.
#[derive(Clone, Copy, Debug)]
pub struct Facts<'a> {
    /// Whether the user turned the move-when-off switch on at all.
    pub enabled: bool,
    /// Whether the user asked for the game and chat channels to be set apart.
    /// Independent of `enabled`: pointing calls at the chat channel is useful
    /// to somebody who never wanted their sound moved anywhere.
    pub split: bool,
    /// The link: `Some(false)` powered down, `Some(true)` up, `None` when the
    /// dongle itself is gone.
    pub link: Option<bool>,
    /// The endpoint id chosen to play through, if one ever was.
    pub fallback: Option<&'a str>,
    /// Whether that endpoint is present on the machine right now.
    pub fallback_present: bool,
    /// The endpoint chosen as the headset's game channel.
    pub game: Option<&'a str>,
    /// Whether that endpoint is present on the machine right now.
    pub game_present: bool,
    /// The endpoint chosen as the headset's chat channel.
    pub chat: Option<&'a str>,
    /// Whether that endpoint is present on the machine right now.
    pub chat_present: bool,
    /// The endpoint currently receiving sound.
    pub current: Option<&'a str>,
    /// The endpoint recorded as owed a move back, if any.
    pub debt: Option<&'a str>,
    /// Whether *that* endpoint is still present. The load-bearing fact.
    pub debt_present: bool,
    /// How many times the endpoints owed have already been looked for.
    pub attempts: u8,
}

/// The decision.
pub fn decide(f: &Facts) -> Action {
    match f.link {
        Some(false) if f.enabled => away(f),
        // Both features have something to say about a headset coming back, so
        // this arm is entered whichever of them is on; `back` sorts out which.
        Some(true) => back(f),
        // A vanished dongle is not a powered-down headset: its endpoints go
        // with it and Windows moves the sound by itself. A powered-down headset
        // with the switch turned off is nothing to do either.
        _ => Action::Nothing,
    }
}

fn away(f: &Facts) -> Action {
    let Some(fallback) = f.fallback else {
        return Action::Blocked(Problem::NoDeviceChosen);
    };
    let Some(current) = f.current else {
        return Action::Nothing;
    };
    if current == fallback {
        // The sound is already there, so there is nothing to move and — more
        // importantly — nothing to owe. Recording a debt here would move the
        // user somewhere they never were when the headset came back.
        return Action::Nothing;
    }
    // A record naming an endpoint the machine still has is a debt that can be
    // paid, so it stands: overwriting it would lose the way back. A record
    // naming one that no longer exists can never be paid by anyone, and must
    // not be the reason this switch is skipped. See the module header.
    if f.debt.is_some() && f.debt_present {
        return Action::Nothing;
    }
    if !f.fallback_present {
        return Action::Blocked(Problem::FallbackAbsent);
    }
    Action::MoveAway {
        to: fallback.to_string(),
        owing: current.to_string(),
    }
}

fn back(f: &Facts) -> Action {
    // The split wins when it is on, and it wins even over a debt that could be
    // paid. Paying the debt would put every role back on one endpoint, which is
    // the very thing the split exists to undo — and the debt is discharged all
    // the same, because the sound lands on the headset either way.
    if f.split {
        return split(f);
    }
    if !f.enabled {
        return Action::Nothing;
    }
    let Some(debt) = f.debt else {
        return Action::Nothing;
    };
    if !f.debt_present {
        return exhausted(f, Action::GiveUp);
    }
    Action::MoveBack {
        to: debt.to_string(),
    }
}

fn split(f: &Facts) -> Action {
    let (Some(game), Some(chat)) = (f.game, f.chat) else {
        // Both or neither. Setting only the role that was chosen would leave
        // the other pointing wherever it happened to point, which is a
        // half-applied setting reported as a working one.
        return Action::Blocked(Problem::NoChannelsChosen);
    };
    if !(f.game_present && f.chat_present) {
        // The link comes up before the endpoints do, every time. This is the
        // ordinary case on the first look, not a failure.
        return exhausted(f, Action::Blocked(Problem::ChannelsAbsent));
    }
    Action::Split {
        game: game.to_string(),
        chat: chat.to_string(),
    }
}

/// Look again, unless this was the last look — in which case, `end`.
fn exhausted(f: &Facts, end: Action) -> Action {
    if f.attempts + 1 >= RESTORE_ATTEMPTS {
        end
    } else {
        Action::Retry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADSET: &str = "{0.0.0.00000000}.{headset-game}";
    const CHAT: &str = "{0.0.0.00000000}.{headset-chat}";
    const SPEAKERS: &str = "{0.0.0.00000000}.{realtek}";
    const MONITOR: &str = "{0.0.0.00000000}.{hdmi}";

    /// Powered on, nothing owed, everything present, split off.
    fn facts() -> Facts<'static> {
        Facts {
            enabled: true,
            split: false,
            link: Some(true),
            fallback: Some(SPEAKERS),
            fallback_present: true,
            game: Some(HEADSET),
            game_present: true,
            chat: Some(CHAT),
            chat_present: true,
            current: Some(HEADSET),
            debt: None,
            debt_present: false,
            attempts: 0,
        }
    }

    /// The same, with the split turned on.
    fn split_facts() -> Facts<'static> {
        Facts {
            split: true,
            ..facts()
        }
    }

    #[test]
    fn powering_off_moves_the_sound_and_records_the_way_back() {
        let f = Facts {
            link: Some(false),
            ..facts()
        };
        assert_eq!(
            decide(&f),
            Action::MoveAway {
                to: SPEAKERS.into(),
                owing: HEADSET.into()
            }
        );
    }

    #[test]
    fn powering_on_moves_the_sound_back_and_clears_the_record() {
        let f = Facts {
            current: Some(SPEAKERS),
            debt: Some(HEADSET),
            debt_present: true,
            ..facts()
        };
        assert_eq!(decide(&f), Action::MoveBack { to: HEADSET.into() });
    }

    /// The regression this module exists for. A record naming an endpoint the
    /// machine no longer has used to block every later switch, permanently and
    /// silently: the debt could not be paid, so it was never cleared, and its
    /// presence was read as "already switched".
    #[test]
    fn a_debt_naming_a_vanished_endpoint_does_not_block_a_new_switch() {
        let f = Facts {
            link: Some(false),
            current: Some(MONITOR),
            debt: Some("{0.0.0.00000000}.{endpoint-that-no-longer-exists}"),
            debt_present: false,
            ..facts()
        };
        assert_eq!(
            decide(&f),
            Action::MoveAway {
                to: SPEAKERS.into(),
                owing: MONITOR.into()
            },
            "a dead record must be replaced, not obeyed"
        );
    }

    #[test]
    fn a_debt_that_can_still_be_paid_is_left_alone() {
        let f = Facts {
            link: Some(false),
            current: Some(MONITOR),
            debt: Some(HEADSET),
            debt_present: true,
            ..facts()
        };
        assert_eq!(
            decide(&f),
            Action::Nothing,
            "the way back must not be overwritten while it is still reachable"
        );
    }

    #[test]
    fn already_on_the_fallback_owes_nothing() {
        let f = Facts {
            link: Some(false),
            current: Some(SPEAKERS),
            ..facts()
        };
        assert_eq!(decide(&f), Action::Nothing);
    }

    /// Even with a dead record in hand: the guard above it must not turn into a
    /// path that records the fallback as the endpoint to return to.
    #[test]
    fn already_on_the_fallback_owes_nothing_even_with_a_dead_record() {
        let f = Facts {
            link: Some(false),
            current: Some(SPEAKERS),
            debt: Some("{gone}"),
            debt_present: false,
            ..facts()
        };
        assert_eq!(decide(&f), Action::Nothing);
    }

    #[test]
    fn a_missing_endpoint_is_retried_and_then_given_up_on() {
        let base = Facts {
            current: Some(SPEAKERS),
            debt: Some(HEADSET),
            debt_present: false,
            ..facts()
        };
        for attempts in 0..RESTORE_ATTEMPTS - 1 {
            assert_eq!(
                decide(&Facts { attempts, ..base }),
                Action::Retry,
                "attempt {attempts} must not be the last word"
            );
        }
        assert_eq!(
            decide(&Facts {
                attempts: RESTORE_ATTEMPTS - 1,
                ..base
            }),
            Action::GiveUp,
            "a debt nothing can discharge must end, or it wedges the feature"
        );
    }

    #[test]
    fn turning_it_on_without_choosing_a_device_says_so() {
        let f = Facts {
            link: Some(false),
            fallback: None,
            fallback_present: false,
            ..facts()
        };
        assert_eq!(decide(&f), Action::Blocked(Problem::NoDeviceChosen));
    }

    #[test]
    fn an_unplugged_fallback_says_so_rather_than_moving_anything() {
        let f = Facts {
            link: Some(false),
            fallback_present: false,
            ..facts()
        };
        assert_eq!(decide(&f), Action::Blocked(Problem::FallbackAbsent));
    }

    #[test]
    fn the_feature_being_off_decides_nothing_at_all() {
        for link in [Some(true), Some(false), None] {
            let f = Facts {
                enabled: false,
                link,
                debt: Some(HEADSET),
                debt_present: true,
                ..facts()
            };
            assert_eq!(decide(&f), Action::Nothing);
        }
    }

    #[test]
    fn a_vanished_dongle_is_not_a_powered_down_headset() {
        let f = Facts {
            link: None,
            debt: Some(HEADSET),
            debt_present: true,
            ..facts()
        };
        assert_eq!(decide(&f), Action::Nothing);
    }

    // ---- the split ---------------------------------------------------------

    /// The bug the split exists for. Restoring one endpoint into every role
    /// took the communications default the user had pointed at the chat
    /// channel and pointed it at the game channel, so the voices came out of
    /// the wrong one.
    #[test]
    fn a_headset_coming_back_gets_calls_on_the_chat_channel() {
        let f = Facts {
            current: Some(SPEAKERS),
            debt: Some(HEADSET),
            debt_present: true,
            ..split_facts()
        };
        assert_eq!(
            decide(&f),
            Action::Split {
                game: HEADSET.into(),
                chat: CHAT.into()
            },
            "the split must beat a plain restore, which is what breaks it"
        );
    }

    /// It is its own feature, not a rider on the move-when-off switch: somebody
    /// who never wanted their sound moved anywhere may still want their calls
    /// on the chat channel.
    #[test]
    fn the_split_works_with_the_move_when_off_switch_turned_off() {
        let f = Facts {
            enabled: false,
            ..split_facts()
        };
        assert_eq!(
            decide(&f),
            Action::Split {
                game: HEADSET.into(),
                chat: CHAT.into()
            }
        );
    }

    #[test]
    fn the_split_does_nothing_while_the_headset_is_off() {
        for link in [Some(false), None] {
            let f = Facts {
                enabled: false,
                link,
                ..split_facts()
            };
            assert_eq!(decide(&f), Action::Nothing, "link {link:?}");
        }
    }

    /// Half a split is not a split: one role set and the other left wherever it
    /// happened to be is a setting that reports success and does the thing the
    /// user was complaining about.
    #[test]
    fn one_channel_without_the_other_is_refused_and_said_out_loud() {
        for (game, chat) in [(Some(HEADSET), None), (None, Some(CHAT)), (None, None)] {
            let f = Facts {
                game,
                chat,
                ..split_facts()
            };
            assert_eq!(
                decide(&f),
                Action::Blocked(Problem::NoChannelsChosen),
                "game {game:?} chat {chat:?}"
            );
        }
    }

    /// The ordinary case on the first look: a wireless link comes up seconds
    /// before its audio endpoints do. Treating that as a failure would report
    /// one on every single power-on.
    #[test]
    fn channels_that_have_not_appeared_yet_are_waited_for() {
        for (game_present, chat_present) in [(false, true), (true, false), (false, false)] {
            let base = Facts {
                game_present,
                chat_present,
                ..split_facts()
            };
            for attempts in 0..RESTORE_ATTEMPTS - 1 {
                assert_eq!(
                    decide(&Facts { attempts, ..base }),
                    Action::Retry,
                    "attempt {attempts} with game {game_present} chat {chat_present}"
                );
            }
            assert_eq!(
                decide(&Facts {
                    attempts: RESTORE_ATTEMPTS - 1,
                    ..base
                }),
                Action::Blocked(Problem::ChannelsAbsent),
                "waiting has to end somewhere"
            );
        }
    }

    /// Every problem has to fit the settings row it is drawn in. That box holds
    /// one line and is vertically centred, so an over-long message wraps
    /// upward into the title rather than downward into empty space.
    #[test]
    fn every_problem_fits_the_settings_row() {
        for p in [
            Problem::NoDeviceChosen,
            Problem::FallbackAbsent,
            Problem::RestoreGone,
            Problem::SwitchFailed,
            Problem::NoChannelsChosen,
            Problem::ChannelsAbsent,
        ] {
            let n = p.short().chars().count();
            assert!(n <= 40, "{:?} is {n} characters: {}", p, p.short());
            assert!(!p.detail().is_empty());
        }
    }

    /// The record outlives the process, so the name written into it has to read
    /// back as the same problem. A key that two variants shared, or one that
    /// nothing parsed, would put a complaint on the wrong settings row or lose
    /// it entirely.
    #[test]
    fn every_problem_survives_a_trip_through_its_key() {
        let all = [
            Problem::NoDeviceChosen,
            Problem::FallbackAbsent,
            Problem::RestoreGone,
            Problem::SwitchFailed,
            Problem::NoChannelsChosen,
            Problem::ChannelsAbsent,
        ];
        for p in all {
            assert_eq!(Problem::from_key(p.key()), Some(p));
        }
        let keys: std::collections::BTreeSet<&str> = all.iter().map(|p| p.key()).collect();
        assert_eq!(keys.len(), all.len(), "keys must be distinct");
        assert_eq!(Problem::from_key("something an older build wrote"), None);
    }
}
