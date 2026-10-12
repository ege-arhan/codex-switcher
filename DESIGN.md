---
name: Codex Switcher
description: Familiar desktop account utility with seven interchangeable themes.
colors:
  macos-neutral-50: "#f5f5f7"
  macos-neutral-100: "#eeeeef"
  macos-neutral-200: "#dedee2"
  macos-neutral-300: "#c8c8ce"
  macos-neutral-500: "#676773"
  macos-neutral-600: "#52525e"
  macos-neutral-700: "#41414b"
  macos-neutral-800: "#2e2e36"
  macos-neutral-900: "#202027"
  macos-neutral-950: "#141419"
  macos-accent: "#006bd6"
  macos-surface: "#fff"
  windows-xp-neutral-50: "#ece9d8"
  windows-xp-neutral-100: "#e5e2d1"
  windows-xp-neutral-200: "#d1cdbb"
  windows-xp-neutral-300: "#b6b19b"
  windows-xp-neutral-500: "#6c675b"
  windows-xp-neutral-600: "#565144"
  windows-xp-neutral-700: "#423f34"
  windows-xp-neutral-800: "#302e27"
  windows-xp-neutral-900: "#24231e"
  windows-xp-neutral-950: "#171711"
  windows-xp-accent: "#1551b7"
  windows-xp-surface: "#fffef8"
  windows-7-neutral-50: "#eaf2f8"
  windows-7-neutral-100: "#e2edf5"
  windows-7-neutral-200: "#c5d8e7"
  windows-7-neutral-300: "#a1bfd4"
  windows-7-neutral-500: "#526c82"
  windows-7-neutral-600: "#415c72"
  windows-7-neutral-700: "#324c61"
  windows-7-neutral-800: "#253b4e"
  windows-7-neutral-900: "#1d2d3c"
  windows-7-neutral-950: "#101e29"
  windows-7-accent: "#11669c"
  windows-7-surface: "#fcfeff"
  terminal-neutral-50: "#e0f5df"
  terminal-neutral-100: "#c9e9c7"
  terminal-neutral-200: "#add6aa"
  terminal-neutral-300: "#91c98c"
  terminal-neutral-500: "#629e5d"
  terminal-neutral-600: "#487944"
  terminal-neutral-700: "#30522d"
  terminal-neutral-800: "#152b18"
  terminal-neutral-900: "#0b1b0e"
  terminal-neutral-950: "#050d07"
  terminal-accent: "#a3de9c"
  paper-neutral-50: "#f5f0e5"
  paper-neutral-100: "#eee7d9"
  paper-neutral-200: "#dbd1be"
  paper-neutral-300: "#c4b79f"
  paper-neutral-500: "#756952"
  paper-neutral-600: "#61563f"
  paper-neutral-700: "#4d4433"
  paper-neutral-800: "#393225"
  paper-neutral-900: "#28241b"
  paper-neutral-950: "#1b1812"
  paper-accent: "#695236"
  paper-surface: "#fffaf0"
  nord-neutral-50: "#eceff4"
  nord-neutral-100: "#e5e9f0"
  nord-neutral-200: "#d8dee9"
  nord-neutral-300: "#bfc9d8"
  nord-neutral-500: "#8192aa"
  nord-neutral-600: "#65758d"
  nord-neutral-700: "#4c566a"
  nord-neutral-800: "#434c5e"
  nord-neutral-900: "#3b4252"
  nord-neutral-950: "#2e3440"
  nord-accent: "#88c0d0"
  mono-neutral-50: "#f4f4f4"
  mono-neutral-100: "#ebebeb"
  mono-neutral-200: "#d5d5d5"
  mono-neutral-300: "#b5b5b5"
  mono-neutral-500: "#606060"
  mono-neutral-600: "#494949"
  mono-neutral-700: "#353535"
  mono-neutral-800: "#242424"
  mono-neutral-900: "#111"
  mono-neutral-950: "#000"
  mono-accent: "#111"
  mono-surface: "#fff"
  codex-active: "#009d71"
  claude-active: "#c36843"
  claude-quota: "#d97757"
typography:
  headline:
    fontFamily: "-apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif"
    fontSize: "18px"
    fontWeight: 600
    lineHeight: "28px"
  title:
    fontFamily: "-apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif"
    fontSize: "16px"
    fontWeight: 600
    lineHeight: "24px"
  body:
    fontFamily: "-apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: "20px"
  label:
    fontFamily: "-apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif"
    fontSize: "12px"
    fontWeight: 500
    lineHeight: "16px"
  provider-tab:
    fontFamily: "-apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif"
    fontSize: "13px"
    fontWeight: 600
    lineHeight: "20px"
rounded:
  macos-radius: "12px"
  macos-control-radius: "7px"
  windows-xp-radius: "6px"
  windows-xp-control-radius: "3px"
  windows-7-radius: "7px"
  windows-7-control-radius: "4px"
  terminal-radius: "0px"
  terminal-control-radius: "0px"
  paper-radius: "2px"
  paper-control-radius: "2px"
  nord-radius: "9px"
  nord-control-radius: "5px"
  mono-radius: "0px"
  mono-control-radius: "0px"
spacing:
  compact: "4px"
  control-gap: "8px"
  row-gap: "12px"
  card: "16px"
  dialog: "20px"
  gutter: "24px"
components:
  button-primary:
    backgroundColor: "{colors.macos-neutral-900}"
    textColor: "{colors.macos-surface}"
    rounded: "{rounded.macos-control-radius}"
    padding: "8px 16px"
  button-primary-hover:
    backgroundColor: "{colors.macos-neutral-800}"
  button-secondary:
    backgroundColor: "{colors.macos-neutral-100}"
    textColor: "{colors.macos-neutral-700}"
    rounded: "{rounded.macos-control-radius}"
    padding: "8px 16px"
  button-claude:
    textColor: "{colors.macos-neutral-800}"
    rounded: "{rounded.macos-control-radius}"
    padding: "8px 12px"
  plan-badge:
    typography: "{typography.label}"
    padding: "4px 8px"
  quota-track:
    backgroundColor: "{colors.macos-neutral-100}"
    height: "6px"
  account-card:
    backgroundColor: "{colors.macos-surface}"
    rounded: "{rounded.macos-radius}"
    padding: "16px"
  provider-tab:
    typography: "{typography.provider-tab}"
    rounded: "{rounded.macos-control-radius}"
    padding: "7px 18px"
  input:
    backgroundColor: "{colors.macos-surface}"
    textColor: "{colors.macos-neutral-900}"
    rounded: "{rounded.macos-control-radius}"
    padding: "8px 12px"
---
# Design System: Codex Switcher

## Overview

**Creative North Star: "Familiar desktop utility"**

Identity, verified remaining quota and the next action establish the hierarchy. The default macOS presentation uses quiet surfaces and system typography, with compact controls and aligned account rows. This is an operational desktop utility inside Tauri, with a React interface.

Windows XP, Windows 7, Terminal, Paper, Nord and Mono are alternate presentations of the same controls. Theme selection changes the neutral ramp, accent, font and geometry together. A separate color-mode control selects light or dark for every identity. It does not change account behavior. The user's explicit familiar OS direction permits system fonts and authentic period chrome.

**Key Characteristics:**
- Identity before actions; remaining quota before advanced detail.
- Seven theme identities, each available in light and dark mode, with a shared component structure.
- Provider accents and labeled status remain independent of theme accents.
- Turkish and English labels, wrapping actions and tabular account numerals.

Evidence: `src/App.css`, `src/App.tsx`, `src/components/AccountCard.tsx`, `src/components/ClaudeAccounts.tsx`, `src/components/UsageBar.tsx`, `src/components/SettingsModal.tsx`, and `src/lib/theme.ts`. Source extraction records implemented behavior; it does not certify every theme's contrast.

## Colors

### Primary

The theme accent is the interaction color: keyboard outlines, caret and selected appearance tile. Frontmatter records each theme's actual values. Most primary account actions use the theme's high contrast neutral rather than filling the interface with the accent.

### Secondary

Codex green marks active account borders and healthy remaining quota. Claude terracotta marks its active account borders, plan treatment and healthy quota; the active-border and quota colors are distinct source values. Amber warns when remaining quota is at or below 30%; red warns at or below 10%. The same threshold rules apply to both providers. Errors and destructive actions use red; warm-up uses amber. These semantic colors are independent of the neutral theme ramps.

### Neutral

The numbered ramp is the shared foundation. Light themes use 50 for the shell, the surface for cards, 200/300 for divisions and borders, 900 for identity, and 500/600 for support copy. All seven identities support dark mode: the shell uses 950, cards use 900, borders use 700, identity uses 100, and support text uses 400/300. The sidecar retains the existing ramp values; no synthetic palette is a runtime token. Some legacy menus still use explicit black or Tailwind neutral utilities; these are implementation exceptions, not an additional palette family.

**The Theme Together Rule.** Apply the selected neutral ramp, font, radii and accent as one presentation. Light/dark controls the surface roles without replacing the theme identity, font or geometry. Dark XP and Aero retain period chrome with adjusted supporting-text contrast; light Terminal uses pale phosphor paper and dark ink; light Nord uses cool pale surfaces and a darker blue accent.

New installations default to macOS light. Existing plain theme IDs retain their historical modes (Terminal and Nord dark; the other five light), and legacy `light`/`dark` values remain supported. Explicit selections persist as `theme:light` or `theme:dark` strings and synchronize to the tray. Selecting a different identity retains the chosen color mode.

**The Provider and State Rule.** Keep provider identity distinct from quota warning and error states; accompany status color with a label or numerical value.

## Typography

**Display Font:** No separate display face or marketing scale.

**Body Font:** The macOS system stack is the default. XP uses Tahoma with Segoe UI fallback; Windows 7 reverses those families. Terminal uses SFMono-Regular, Consolas and Liberation Mono. Paper uses Georgia and Times New Roman. Nord and Mono inherit the default system stack.

**Character:** Compact desktop typography establishes hierarchy through weight and proximity. Account cards use tabular numerals so percentages and refresh information remain stable while updating.

### Hierarchy

- **Headline:** The application title and dialog/provider headings use the headline role; Paper reduces h1/h2 weight to 500.
- **Title:** Account identity is semibold at the inherited base size. Codex names truncate within their row; Claude identities can break across lines.
- **Body:** Actions, descriptions and inputs use the body role; medium weight marks actions.
- **Label:** Quota rows, timestamps and plan badges use the label size, with regular weight for secondary information.
- **Provider tab:** The explicit stylesheet role overrides the utility font size and weight.

**The Utility Hierarchy Rule.** Reserve emphasis for identity, dialog titles and actionable state; keep percentages and reset times readable alongside their labels.

## Layout

The main content and toolbar share a centered maximum width of 64rem with 24px horizontal gutters. The main content starts with 16px top padding and 32px bottom padding. The toolbar has a 96px minimum height until the 760px compact breakpoint; at the 768px utility breakpoint it places identity and actions in two columns, otherwise they stack. Controls and card action rows wrap.

Accounts form one column with 12px gaps and 16px internal padding. Shared spacing progresses through compact, control-gap, row-gap, card, dialog and gutter tokens. Active and other account groups remain explicit. Search spans the content width. Advanced settings live in a dialog up to 32rem wide, with 16px outer margins, 20px sections and a scrolling body capped at 70vh.

Theme identity choices form two columns with 8px gaps, becoming one column at 420px. A separate two-option Light/Dark radio group applies to every theme. The provider navigation and automatic warm-up control share a wrapping header row. Long translated labels must use the existing wrapping behavior rather than fixed text widths. Dates and numbers use the active locale.

## Elevation & Depth

Borders and tonal surfaces supply most separation. Active account emphasis is primarily a provider border. Selected provider tabs use a subtle short shadow. Menus and dialogs retain larger utility shadows over the dialog's 40% black scrim. Paper gives cards a small warm shadow; XP uses inset button bevels and a one-pixel card highlight; Windows 7 uses soft inset highlights and shallow blue-gray elevation. Terminal and Mono explicitly remove card shadows.

### Shadow Vocabulary

- **Selected provider:** `0 1px 2px #0000000a`.
- **XP control bevel:** `inset 1px 1px #ffffff9c, inset -1px -1px #0000001a`.
- **Windows 7 control:** `inset 0 1px #ffffffa6, 0 1px 1px #244d6b16`.
- **Windows 7 card:** `0 1px 3px #244d6b12`.
- **Paper card:** `0 2px 4px #4d443314`.

**The Period Chrome Rule.** Keep the existing XP and Aero gradients and bevels in their period themes; use the other themes' own flat or tonal materials elsewhere.

## Shapes

Card/dialog and control radii are separate theme tokens. macOS is gently curved; XP and Windows 7 use tighter corners; Paper is almost square; Nord is softly rounded; Terminal and Mono are square. Plan badges and quota tracks retain their pill geometry across themes. Control radii do not replace the geometry of every non-control surface.

The Codex identity is a text-like rename control. XP and Windows 7 deliberately exclude it from their framed/beveled button rules. Provider navigation has a compact inset track, while the selected segment is surfaced. Outlined SVGs supply action icons.

## Components

### Buttons

Primary Codex actions use a dark neutral fill with light text, reversing their light/dark roles in dark mode where the component defines that reversal. Secondary actions use a tonal background; provider-specific Claude actions are outlined. Common account actions have 8px vertical padding. Toolbar controls are 40px high. Icon-only refresh and privacy controls have accessible labels and outlined SVGs. Disabled actions reduce opacity to 50% where defined, while the active-account action uses a muted labeled state.

Hover changes tone or border, without decorative translation. The global keyboard outline is a 2px theme-accent stroke offset by 3px. Claude action buttons add their terracotta focus outline. Some inputs add their own utility focus rings; preserve the global visible outline too.

### Chips

Plan and automation chips are compact bordered labels. Provider, paid-plan and automation treatments use semantic color with explicit text. Active state combines a provider border, status label and, on Codex cards, a small status dot. Dots do not replace status text.

### Cards / Containers

Cards use the theme surface, theme card radius, thin border and card spacing token. Header and action rows wrap independently. Codex rename controls and privacy masking stay attached to identity; Claude cards disclose login state and verification. Account timestamps use supporting text rather than the faint loading-text treatment. Advanced statistics and project details are disclosed with labeled controls.

### Inputs / Fields

Settings fields use a full-width surface, thin neutral border, theme control radius and 8px by 12px padding. Search uses an inset SVG and extra horizontal room for its clear action. Carets follow the theme accent. Focus is visibly outlined; disabled fields use reduced opacity. Theme options contain real radio inputs, with focus reflected on the visible label and selection indicated by an accent border plus inset stroke.

### Navigation

Provider navigation is a compact segmented control with 3px track padding and gap. Selected segments use a surfaced fill, border and short shadow; Terminal uses light phosphor on dark, and Mono uses black on white. Buttons expose their pressed state and remain keyboard accessible. Both provider screens share one header: identity, automatic-switch toggle, privacy, refresh, warm-up, menu, settings and account controls remain in the same positions. The provider segments include account counts and share their row with automatic warm-up. Commands route to the selected provider; each provider retains its own automation state. Claude import and account creation live in the common account menu, while Code/Desktop launch and sign-in details stay attached to account cards. Open Codex and running Codex process controls remain provider-specific. The shared timer menu retains Codex-only scheduling because Claude has no corresponding scheduled-warm-up command.

### Remaining quota

A 6px rounded track carries the remaining percentage, window label and reset time. Healthy color follows the provider; warning thresholds supersede it. Each bar exposes a progressbar name, minimum, maximum and remaining value. Missing verification, loading and errors appear as text rather than invented percentage values. Quota width transitions use 300ms; identity masking uses 200ms and card border/color changes use 150ms. Reduced-motion preference disables transitions, animations and smooth scrolling globally.

## Do's and Don'ts

### Do:
- **Do** switch theme tokens together and preserve provider and warning semantics.
- **Do** keep identity, remaining quota and the next action visible before advanced settings.
- **Do** preserve visible keyboard focus, explicit status labels and locale-aware values.
- **Do** allow action rows and translated copy to wrap inside the shared content width.

### Don't:
- **Don't** spread XP or Aero chrome into the other themes.
- **Don't** frame the account identity as an XP or Aero command button.
- **Don't** infer verified quota from a loading or unknown state.
- **Don't** use real account identities or credentials in documentation examples.

Not canonized: the build's 10–11px tertiary helper text and faint loading placeholders are inherited density/legibility debt, not reusable text roles; explicit black/neutral menu exceptions are not a second theme system. The OS screen-sharing overlay is outside the application. Documentation follows the implemented source. Theme state tests cover all fourteen identity/mode combinations and legacy selections; a complete independent visual/contrast audit is not implied.

Both providers group the selected Code account under Active account, followed by Other accounts. Claude keeps its separate Desktop profile label; provider color marks the active card in both appearances.
