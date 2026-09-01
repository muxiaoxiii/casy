---
name: Slate 2.0
colors:
  surface: '#f7f9fb'
  surface-dim: '#d8dadc'
  surface-bright: '#f7f9fb'
  surface-container-lowest: '#ffffff'
  surface-container-low: '#f2f4f6'
  surface-container: '#eceef0'
  surface-container-high: '#e6e8ea'
  surface-container-highest: '#e0e3e5'
  on-surface: '#191c1e'
  on-surface-variant: '#444750'
  inverse-surface: '#2d3133'
  inverse-on-surface: '#eff1f3'
  outline: '#747781'
  outline-variant: '#c4c6d1'
  surface-tint: '#3f5d9b'
  primary: '#244481'
  on-primary: '#ffffff'
  primary-container: '#3e5c9a'
  on-primary-container: '#c8d7ff'
  inverse-primary: '#afc6ff'
  secondary: '#595e6c'
  on-secondary: '#ffffff'
  secondary-container: '#dee2f3'
  on-secondary-container: '#5f6472'
  tertiary: '#613f00'
  on-tertiary: '#ffffff'
  tertiary-container: '#7d5610'
  on-tertiary-container: '#ffcf8c'
  error: '#ba1a1a'
  on-error: '#ffffff'
  error-container: '#ffdad6'
  on-error-container: '#93000a'
  primary-fixed: '#d9e2ff'
  primary-fixed-dim: '#afc6ff'
  on-primary-fixed: '#001a43'
  on-primary-fixed-variant: '#254582'
  secondary-fixed: '#dee2f3'
  secondary-fixed-dim: '#c2c6d6'
  on-secondary-fixed: '#161b27'
  on-secondary-fixed-variant: '#424754'
  tertiary-fixed: '#ffddb1'
  tertiary-fixed-dim: '#f3bd6f'
  on-tertiary-fixed: '#291800'
  on-tertiary-fixed-variant: '#624000'
  background: '#f7f9fb'
  on-background: '#191c1e'
  surface-variant: '#e0e3e5'
  slate-blue-deep: '#1F2430'
  slate-blue-muted: '#4B5160'
  slate-gray-light: '#9BA2AF'
  surface-white: '#FFFFFF'
  border-default: '#E4E7EB'
  status-risk: '#B4554F'
  status-success: '#4C8067'
  status-warning: '#B0823A'
  status-discovery: '#6C6A9C'
  bg-risk-weak: '#F7EFEE'
  bg-success-weak: '#EDF4F0'
  bg-warning-weak: '#F6F3EA'
typography:
  display-date:
    fontFamily: Inter
    fontSize: 20px
    fontWeight: '700'
    lineHeight: 28px
    letterSpacing: -0.3px
  headline-section:
    fontFamily: Inter
    fontSize: 15px
    fontWeight: '700'
    lineHeight: 20px
  header-app:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: '700'
    lineHeight: 20px
  body-base:
    fontFamily: Inter
    fontSize: 13px
    fontWeight: '400'
    lineHeight: 18px
  body-medium:
    fontFamily: Inter
    fontSize: 13px
    fontWeight: '500'
    lineHeight: 18px
  ui-subtext:
    fontFamily: Inter
    fontSize: 12.5px
    fontWeight: '500'
    lineHeight: 16px
  label-caps:
    fontFamily: Inter
    fontSize: 11px
    fontWeight: '600'
    lineHeight: 14px
  mono-data:
    fontFamily: SF Mono
    fontSize: 11px
    fontWeight: '500'
    lineHeight: 14px
  mono-micro:
    fontFamily: SF Mono
    fontSize: 10.5px
    fontWeight: '600'
    lineHeight: 12px
rounded:
  sm: 0.25rem
  DEFAULT: 0.5rem
  md: 0.75rem
  lg: 1rem
  xl: 1.5rem
  full: 9999px
spacing:
  base: 8px
  half: 4px
  quarter: 2px
  container-gap: 14px
  margin-page: 20px
  sidebar-width: 212px
  preview-width: 440px
---

## Brand & Style

The design system embodies the "Professional Steward"—a quiet, reliable, and highly precise interface designed for legal experts. It prioritizes information clarity, source traceability, and cognitive ease over decorative flair.

The aesthetic follows a **Corporate / Modern** approach with a **Minimalist** lean. It avoids unnecessary shadows, vibrant gradients, or personification. The visual language is "Quiet," using color and depth only to signal risk, status, and structural hierarchy. The atmosphere is one of focused diligence, where the interface stays out of the way until a critical deadline or action requires attention.

**Key Principles:**
- **Visual Quiet:** High whitespace and muted tones to reduce fatigue during long-duration expert work.
- **Semantic Honesty:** Every color and shape represents a specific legal fact or system state.
- **Source Traceability:** Visual cues distinguish between AI-suggested and user-confirmed data.
- **Deterministic Kernel:** The core layout remains functional and structured even without active AI enhancements.

## Colors

This design system utilizes a "Slate" palette designed for professional reliability. Color is used as a functional tool for status and risk assessment rather than decoration.

- **Primary Action:** `#3E5C9A` (Slate Blue) is reserved for focal points and primary interactive states.
- **Surface System:**
  - **Layer 0 (App Background):** `#F8FAFC` — Provides a cool, professional base.
  - **Layer 1 (Cards/Surfaces):** `#FFFFFF` — High-contrast surfaces for primary work.
  - **Layer 2 (Overlays):** Utilizes subtle borders (`#E4E7EB`) and deep shadows for modals and popovers.
- **Semantic Logic:**
  - **Risk/Hard Deadlines:** Muted Ruby (`#B4554F`) used for critical alerts.
  - **Waiting/Pending:** Subtle Amber (`#B0823A`) for warnings or follow-up points.
  - **Completion:** Soft Emerald (`#4C8067`) for successful synchronization and finished tasks.
- **Accessibility:** Color must never be the sole carrier of information; always pair semantic colors with icons or descriptive labels.

## Typography

Typography focuses on information density and hierarchy. **Inter** handles all UI elements and body text to ensure modern readability, while **SF Mono** is strictly used for technical data like Case IDs, timestamps, and file paths.

**Hierarchy Rules:**
- **Alignment Over Size:** Use strict vertical alignment and grouping to establish hierarchy rather than excessive font size variations.
- **Data Density:** The system uses smaller base sizes (13px) to accommodate the complex, multi-dimensional tables required for legal work.
- **Mobile Adaptation:** Headlines larger than 18px scale down by 15% on mobile devices to preserve screen real estate.

## Layout & Spacing

The layout is built on an **8px grid system** with a specific **14px core rhythm** for internal component padding and spacing between cards.

**Layout Philosophy:**
- **Three-Tier Architecture:** 
  1. **Global Focus:** The top-level "Focus Desk" for immediate commitments.
  2. **Context Layer:** Middle-level work surfaces (Case Workbenches).
  3. **Tool Layer:** Bottom-level utilities (Global Search, Audit Logs).
- **Fixed Widths:** The Sidebar (212px) and Preview Panel (440px) remain fixed to provide a stable "Context Return" experience where users don't lose their place while navigating back.
- **Responsive Behavior:** On tablet and mobile, the Preview Panel transitions to a full-screen overlay, while the Sidebar collapses into a drawer.

## Elevation & Depth

The system uses a **Layered Context** approach rather than traditional shadows. Depth is conveyed through tonal layering and hair-line borders.

- **Flat Surfaces:** Cards and primary work surfaces use a 1px border (`#E4E7EB`) instead of shadows to maintain a clean, "quiet" aesthetic.
- **Modality:** Modals and Popovers (Layer 2) use a soft, diffused shadow (`0 8px 32px rgba(20, 24, 35, 0.14)`) to separate them from the work surface.
- **Interaction Depth:** Hover states are indicated by a slightly stronger border (`#CCD0D8`) rather than an elevation increase.
- **Spatial Logic:** Side panels slide in from the right (`0.18s ease`), visually indicating they are an extension of the current context rather than a new page.

## Shapes

Shapes are strictly functional, used to distinguish between different data "primitives" (Actions, Events, and Deadlines).

- **Card Radius:** 8px (`rounded-lg`) for main content containers to provide a soft, professional feel.
- **Button Radius:** 6px for UI controls, creating a subtle visual distinction from card containers.
- **Input Radius:** 4px for form fields to maintain a precise, structured look.
- **Pill Radius:** 999px for status badges and audit tags, ensuring they are immediately recognizable as secondary metadata.
- **Checkboxes:** 4px radius with a 1.5px solid border.

## Components

### Case Track Status Indicators
Visualized as parallel progress bars for the "Three-Track" state machine:
- **Civil:** Primary Blue.
- **Invalidity:** Purple (`#6C6A9C`).
- **Administrative:** Slate Gray.
- *Style:* 4px height tracks with rounded ends.

### Time-Twin Blocks
Used to separate user intention from legal reality:
- **When (User):** Left-aligned, standard weight Inter.
- **Deadline (Fact):** Right-aligned, SF Mono, colored semantically by risk (Muted Ruby if < 24h).

### Next Action Cards
Cards containing the most urgent next step with progress context.
- *Style:* White background, 8px radius, primary left-accent border (4px width) using the color of the associated legal track.

### Audit & Traceability Badges
- **AI-Suggested:** Dashed border with a subtle "AI" prefix in `mono-micro` type.
- **User-Confirmed:** Solid background with a checkmark icon.
- *Note:* AI-generated content must always include a "Degrade Bar" or "Source Citation" link.

### Buttons & Inputs
- **Primary Button:** `#3E5C9A` fill, white text, 6px radius.
- **Secondary/Ghost:** No fill, `#4B5160` text, 1px border on hover.
- **Input Fields:** `#FFFFFF` fill, `#E4E7EB` border, 4px radius, 13px Inter text.