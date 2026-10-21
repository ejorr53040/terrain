# A Spread needs no Engagement, and sends a key chord only

A Spread (two hands brought together, then spread apart quickly) toggles full screen with Super+F. It runs on any two hands in view, with no hand having control. This is an exception to ADR 0002, which keeps Engagement built in so that hands typing at the keyboard can't start clicking. It's allowed because a Spread can't happen at the keyboard: it needs both palms within 20 cm for 150 ms, then more than 32 cm apart within 450 ms. The recorded take was done low in front of the chest, with neither hand raised into the Reach, and asking for Engagement first would have meant changing how it is naturally done. To keep ADR 0002's promise anyway, a Spread's binding is a key chord (`keys: [Super, F]`), pressed in order and released in reverse on the same frame. Mouse buttons are refused when the gesture file is parsed, and nothing stays held after the chord.

## Considered options

- Require a hand with control first: safe under ADR 0002, but it forces raising a hand before every Spread.
- Let a Spread bind any actions (the first version, `enter: [...]`): a gesture file could then make hands at the keyboard click or hold a key.
- A Fist pushed toward the camera (the first full-screen gesture): dropped 2026-10-12. On a single webcam it kept reading as a window drag (`docs/research/hand-depth.md`).

## Consequences

- A Spread is only considered while neither hand is in a one-hand gesture. It is checked after the hand with control has been stepped, so a gesture that hand starts on the same frame already blocks it.
- Hands meeting often read as pinching, so both Open and Pinch count.
- A hand that drops out of tracking at the edge of the view mid-spread keeps the Spread armed, for at most 450 ms.
- Thresholds come from one recorded take (`full_screen_motion`), so the margins are thin: that take's slowest spread took 400 ms, while a 1.5 s drift apart takes 530 ms.
- A clap held for 150 ms and then dropped fast to a wide rest would toggle.
