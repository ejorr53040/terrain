# Bindings can't run commands, and Engagement can't be rebound

A binding's action is limited to pointer movement, mouse buttons, scroll and key chords. Running shell commands was rejected because the checker can't reason about it, and a misread pose would become arbitrary execution. Engagement (raise an open hand into the reach to take control, lower it to give control up) is built in and not part of the gesture file, so a bad file can't lock the user out or let hands typing at the keyboard start clicking.
