# Windows wheel integration

Source: `nsh-platform-windows` 0.7.2 on crates.io, from the project’s pinned NSH commit `0c29b5e3351128f489fe53bfb167160801707191`. The Windows portable launcher uses this copy. The single change in `src/scroll.rs::wheel_notch` allows a scroll view explicitly configured with `smoothWheel` to use NSH’s existing compositor-paced wheel curve when Windows disables client-area animations. The app’s wheel preference controls the property. With the preference off, wheel events follow the original Windows behavior. Touchpad deltas, keyboard, dialogs and other animations use the upstream implementation.

For an upgrade, compare this function against the new release, then run `scripts/measure-windows-scroll.ps1` with both a stepped wheel and precision input. The desktop pixel sampler checks the actual visible output separately from the NSH frame clock.
