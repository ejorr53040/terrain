# A Spread needs no Engagement, and sends a key chord only

A Spread (two hands brought together, then spread apart quickly) toggles full screen with Super+F. It runs on any two hands in view, with no hand having control. This is an exception to ADR 0002, which keeps Engagement built in so that hands typing at the keyboard can't start clicking. It's allowed because a Spread can't happen at the keyboard: it needs both palms within 20 cm for 150 ms, then more than 26 cm apart within 450 ms, no more than 200 ms of it with both hands in view. The recorded take was done low in front of the chest, with neither hand raised into the Reach, and asking for Engagement first would have meant changing how it is naturally done. To keep ADR 0002's promise anyway, a Spread's binding is a key chord (`keys: [Super, F]`), pressed in order and released in reverse on the same frame. Mouse buttons are refused when the gesture file is parsed, and nothing stays held after the chord.

## Considered options

- Require a hand with control first: safe under ADR 0002, but it forces raising a hand before every Spread.
- Let a Spread bind any actions (the first version, `enter: [...]`): a gesture file could then make hands at the keyboard click or hold a key.
- A Fist pushed toward the camera (the first full-screen gesture): dropped 2026-10-12. On a single webcam it kept reading as a window drag (`docs/research/hand-depth.md`).

## Consequences

- A Spread is only considered while neither hand is in a one-hand gesture. It is checked after the hand with control has been stepped, so a gesture that hand starts on the same frame already blocks it.
- Hands meeting often read as pinching, so both Open and Pinch count.
- While a Spread's palms are together, or parted within the last 450 ms, the hand with control can't start a gesture: its pinch is the palms meeting. Found live (2026-10-21). At chest height one hand took control, its pinch as the palms met started a pinch gesture, and that blocked the Spread and clicked or dragged instead. The cost: a pinch made with the other hand within 20 cm of it does nothing.
- A hand that drops out of tracking at the edge of the view mid-spread keeps the Spread armed, for at most 450 ms. So does a hand that briefly reads as another Pose: live, a hand opening to spread passed through a Vee for one frame. It fires only with both hands in an allowed Pose.
- Thresholds come from two recorded takes, so the margins are thin. In `full_screen_motion` the slowest spread took 400 ms, while a 1.5 s drift apart takes 530 ms. `spread_short` (2026-10-21) parted only to 28-31 cm, so `apart` dropped from 32 cm to 26 cm; until then most live Spreads missed.
- A clap held for 150 ms and then dropped fast to a wide rest would toggle.
- A slow drift apart is told from a Spread by time with both hands seen, not wall time: a hand flung out of view drops out for 300-370 ms, longer than a drift takes, but a dropout stops the clock. Needed (2026-10-21) once `apart` dropped to 26 cm.
- A Spread stays armed after it fires, until 450 ms after the palms were last together, so a pinch lingering from the palms meeting can't start a click.
- Hands in a Spread's Poses closer than `near` (35 cm) count as a Spread getting under way, so the hand with control can't start a gesture. Found live (2026-10-22, #85): fingertips meeting read as pinching from about 30 cm apart, before the palms were within `together`; the pinch clicked, held as a drag, and blocked the Spread. Like the arming, a dropout or a stray Pose keeps this for up to 450 ms, or a pinch held through a dropout would click at once. The cost grows: while the other hand is Open or Pinch within 35 cm (was 20 cm), the hand with control can't start any gesture, including with both hands resting 25–30 cm apart between Spreads. Zoom (#75) as specced starts from the hand with control's pinch at 25 cm or more apart; inside 35 cm that pinch can't start, so Zoom must start at 35 cm or more, or not depend on that pinch.

## Amendment (2026-10-21): Bump

A Bump (two fists brought together) takes a screenshot (Omarchy's Super+Shift+R) on the same terms: any two hands, no Engagement, a key chord only. Like the palms of a Spread, fists meeting can't happen at the keyboard. The Spread consequences carry over with fists for palms: a Bump is only considered while neither hand is in a one-hand gesture, and while the fists are together the hand with control can't start a gesture (its still fist would otherwise sit in the "✊ Fist" state and block the Bump). A Bump fires once and re-arms when the fists part or a hand opens.

Zoom, the other two-hand gesture added at the same time, does not get this exception. Two hands pinching well apart can happen at the keyboard, so Zoom needs a hand with control.
