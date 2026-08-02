# Light Theme Design

Status: approved 2026-08-02.

## Problem

The panel has one palette, sampled from dark mockups. On a machine set to light, it is a
dark rectangle that matches nothing around it.

## Goals

- A light palette, sampled from the supplied mockups rather than invented.
- The user's Windows preference honoured by default, with a manual override.
- No new concepts in the UI: the control reuses a pattern the panel already teaches.

## Non-goals

- Theming the installer or the CLI.
- A custom accent colour, or any palette the user can edit.
- Changing the dark palette. It stays exactly as sampled.

## Precedence

Three inputs decide the palette, in this order:

1. **High contrast wins.** It is already implemented and already overrides. A user who
   turned it on did not mean "unless the app has a light theme".
2. **An explicit override**, if the user set one.
3. **Windows' own preference**, which is the default.

## Selection and storage

An `Appearance` string value under `HKCU\Software\HeadsetTray`, alongside the existing
`ShowSynapseWarning` — the same storage the settings panel already uses:

| Value | Meaning |
| --- | --- |
| absent or `system` | Follow Windows. **The default.** |
| `light` | Always light |
| `dark` | Always dark |

Windows' preference is `AppsUseLightTheme` under
`HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize`: `1` is light, `0` is
dark, and **absent is treated as dark**, matching what Windows itself does.

## The control

A new **Appearance** row in the Settings view, below the two existing toggles, using the
**same three-segment control as the noise mode row**:

```
  Appearance                     ┌──────┬──────┬──────┐
  Following Windows — dark       │ AUTO │  ☾   │  ☀   │
                                 └──────┴──────┴──────┘
```

Reusing that component is the point. The panel already teaches "a row of segments, the
active one filled with the accent" for noise mode; a second instance costs the user nothing
to learn. The existing settings rows are pill toggles, which cannot express three states.

The subtitle states what actually resolved — `Following Windows — dark`, `Light theme`,
`Dark theme` — so `AUTO` is not a black box. A user whose Windows is dark and who picks
`AUTO` should be able to see why the panel stayed dark.

## Palette

### Sampled from the mockups

Measured by analysing colour frequency across both images rather than by reading fixed
coordinates. Guessed coordinates were tried first and landed on background — the mockups
are 363×457 and 356×451, different from the 358×521 dark ones, so positions do not carry
over.

| Role | Light | Dark, for comparison |
| --- | --- | --- |
| `bg_panel` | `#F0F0F5` | `#131623` |
| `bg_card` | `#E8E8EE` | `#1A1D29` |
| `border_card` | `#D2D2D8` | `#272935` |
| `text_primary` | `#23252F` | `#E9E9ED` |
| `text_secondary` | `#9397AB` | `#8E91A6` |
| `accent` | `#6153B8` | `#9184D9` |
| `state_live` | `#389669` | `#4ECB89` |

The accent and the live-green both **darken** for light mode. That is not a stylistic
choice being copied; it is what the mockups show, and it is what contrast against a light
background requires.

### Present in the mockups but not yet located

Three further greys appear in quantity — `#D4D4DC`, `#DCDDE4`, `#D2D2D8` — which are the
gear button, the off-state toggle track, and the card border in some order. Implementation
assigns them by **locating the feature and sampling it**, not by guessing which is which.

### Derived, and marked as such

Four roles appear in neither mockup, because neither shows the state that uses them:
`bg_banner`, `border_banner`, `text_muted`, `state_muted`.

They are derived from the relationships the dark palette already uses, and `theme.rs` must
mark them as derived rather than measured — the file opens by saying every value in it was
sampled, and that claim has to stay true.

The warning banner is the one worth stating explicitly. In the dark palette it is `#1D1E31`
against a `#1A1D29` card: **barely distinguishable**. The warning is carried by the ⚠ glyph
and the wording, not by colour. The light banner keeps that relationship rather than
introducing an amber the mockups never specified.

## Reacting to changes

`WM_SETTINGCHANGE` is already handled for high contrast. It extends to re-reading the
Windows preference, so a user switching Windows to light while the panel is open sees it
follow — provided they are on `AUTO`.

## Testing

- **Resolution is a pure function** of `(setting, windows preference, high contrast)`,
  covered by a table test over all combinations. This is where the precedence rule lives,
  and it is the part most likely to be got wrong.
- **The light palette gets the same luminance-gap check the high-contrast one has**, over
  every foreground/background pair that actually sits on top of another, so unreadable text
  cannot be introduced by a later edit.
- **`--render-panel` gains light fixtures**, so the theme is diffable like every other panel
  state, and the dark fixtures must stay byte-identical.

## Notes for whoever tests it

The development machine is set to dark, so `AUTO` resolves to dark there. Seeing light mode
means choosing the sun, or switching Windows and watching `AUTO` follow.

The mockups are not committed, consistent with the dark ones. What is committed is the
sampled values and the record of how they were obtained.
