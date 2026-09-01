---
name: Luminous Terra
colors:
  surface: '#fcf9f2'
  surface-dim: '#dcdad3'
  surface-bright: '#fcf9f2'
  surface-container-lowest: '#ffffff'
  surface-container-low: '#f6f3ec'
  surface-container: '#f0eee7'
  surface-container-high: '#ebe8e1'
  surface-container-highest: '#e5e2db'
  on-surface: '#1c1c18'
  on-surface-variant: '#414844'
  inverse-surface: '#31312c'
  inverse-on-surface: '#f3f0ea'
  outline: '#727974'
  outline-variant: '#c1c8c2'
  surface-tint: '#446556'
  primary: '#416353'
  on-primary: '#ffffff'
  primary-container: '#597c6b'
  on-primary-container: '#f5fff7'
  inverse-primary: '#aacfbc'
  secondary: '#3f6375'
  on-secondary: '#ffffff'
  secondary-container: '#c0e5fb'
  on-secondary-container: '#44677a'
  tertiary: '#805041'
  on-tertiary: '#ffffff'
  tertiary-container: '#9c6858'
  on-tertiary-container: '#fffbff'
  error: '#ba1a1a'
  on-error: '#ffffff'
  error-container: '#ffdad6'
  on-error-container: '#93000a'
  primary-fixed: '#c5ebd7'
  primary-fixed-dim: '#aacfbc'
  on-primary-fixed: '#002115'
  on-primary-fixed-variant: '#2c4d3f'
  secondary-fixed: '#c3e8fd'
  secondary-fixed-dim: '#a7cce1'
  on-secondary-fixed: '#001e2b'
  on-secondary-fixed-variant: '#264b5d'
  tertiary-fixed: '#ffdbd1'
  tertiary-fixed-dim: '#f7b8a5'
  on-tertiary-fixed: '#331107'
  on-tertiary-fixed-variant: '#683b2e'
  background: '#fcf9f2'
  on-background: '#1c1c18'
  surface-variant: '#e5e2db'
  surface-cream: '#FCF9F2'
  surface-canvas: '#F5F2EA'
  text-primary: '#2D3436'
  text-secondary: '#636E72'
  sage-muted: '#8BA899'
  border-ethereal: rgba(0, 0, 0, 0.06)
typography:
  headline-lg:
    fontFamily: Libre Franklin
    fontSize: 32px
    fontWeight: '700'
    lineHeight: 40px
    letterSpacing: -0.01em
  headline-lg-mobile:
    fontFamily: Libre Franklin
    fontSize: 26px
    fontWeight: '700'
    lineHeight: 32px
  headline-md:
    fontFamily: Libre Franklin
    fontSize: 24px
    fontWeight: '600'
    lineHeight: 32px
  headline-sm:
    fontFamily: Libre Franklin
    fontSize: 20px
    fontWeight: '600'
    lineHeight: 28px
  body-lg:
    fontFamily: Source Serif 4
    fontSize: 18px
    fontWeight: '400'
    lineHeight: 30px
  body-md:
    fontFamily: Source Serif 4
    fontSize: 16px
    fontWeight: '400'
    lineHeight: 26px
  body-sm:
    fontFamily: Source Serif 4
    fontSize: 14px
    fontWeight: '400'
    lineHeight: 22px
  label-md:
    fontFamily: Libre Franklin
    fontSize: 12px
    fontWeight: '600'
    lineHeight: 16px
    letterSpacing: 0.05em
  label-sm:
    fontFamily: Libre Franklin
    fontSize: 11px
    fontWeight: '600'
    lineHeight: 14px
    letterSpacing: 0.03em
rounded:
  sm: 0.125rem
  DEFAULT: 0.25rem
  md: 0.375rem
  lg: 0.5rem
  xl: 0.75rem
  full: 9999px
spacing:
  unit: 4px
  gutter: 24px
  margin-mobile: 16px
  margin-desktop: 64px
  stack-sm: 8px
  stack-md: 16px
  stack-lg: 32px
---

## Brand & Style

The design system evolves from a grounded aesthetic into a **Luminous Terra** experience, specifically tailored for high-stakes legal professionals who require clarity without the clinical coldness of standard SaaS tools. The brand personality is "Aerated Organic"—it feels breathable, intellectual, and premium.

The visual direction is **Minimalism with a Solarized Light aesthetic**. It prioritizes extreme "eye-comfort" for long-duration legal drafting and research. By shifting from heavy, saturated tans to airy, light-cream surfaces, the system achieves a sense of "open space" even when displaying dense data. 

**Key Brand Pillars:**
- **Intellectual Clarity:** High-contrast typography on off-white surfaces ensures peak readability.
- **Organic Professionalism:** A muted, sage-inspired palette that feels natural but remains strictly corporate.
- **Weightless Structure:** Reducing container visual weight to allow the user's focus to drift toward the content, not the interface chrome.

## Colors

The palette is a refined, airy interpretation of the Solarized Light theory, optimized for legal readability and a "fresh parchment" feel.

- **Primary Backgrounds:** The core interface uses a very light cream (`#FCF9F2`). This reduces the yellow-heavy intensity of the previous version, providing a brighter, more "bleached" canvas that feels modern.
- **Primary Green:** The primary action color has been softened to a muted Sage/Moss green (`#6B8E7D`). It maintains professional authority but feels more organic and less aggressive.
- **Secondary & Tertiary:** A desaturated slate blue and a soft terracotta are used for subtle categorization and citation highlighting.
- **Typography:** To maintain legal-grade contrast, the base body text uses a very dark charcoal (`#2D3436`) rather than a teal-tinted gray, ensuring maximum legibility against the cream background.
- **Containers:** Backgrounds for sidebars or secondary panels use a slightly cooler off-white (`#F5F2EA`) to provide distinction without high-contrast jumps.

## Typography

This design system moves away from a purely sans-serif approach to embrace a hybrid system better suited for long-form legal reading.

- **Headlines & UI Labels:** **Libre Franklin** provides a sturdy, professional, and contemporary sans-serif skeleton for structural elements. It is reliable and highly legible at various weights.
- **Body & Document Content:** **Source Serif 4** is utilized for primary reading content. This serif face provides the "intellectual" feel required for legal work while significantly improving reading stamina on warm, light-mode screens.
- **Spacing:** Body copy features slightly expanded line-heights (1.6x-1.7x) to contribute to the overall "airy" feel of the system.

## Layout & Spacing

The layout philosophy follows a **Fixed Grid** for reading and a **Fluid Grid** for navigation, emphasizing whitespace as a functional element.

- **Margins:** Desktop margins are increased to 64px to create a centered "column of focus" for legal documents, simulating the luxury of a wide-margined physical page.
- **Rhythm:** A strict 4px base unit is used. To enhance the "airy" feel, internal container padding is prioritized over external margins, pushing content away from borders to create "breathing room."
- **Reflow:** On mobile, the grid collapses to 4 columns with tighter 16px margins, but maintains the 24px gutter to ensure elements never feel crowded.

## Elevation & Depth

This design system rejects heavy shadows in favor of **Low-Contrast Outlines** and **Tonal Layering**, creating a flatter, cleaner aesthetic.

- **Surface Tiers:** Hierarchy is defined by color shifts rather than height. The base layer is `#FCF9F2`. Secondary "wells" or sidebars use `#F5F2EA`.
- **Ethereal Borders:** Containers are defined by 1px borders using `border-ethereal` (a very soft, 6% opacity black). This provides enough structure to distinguish sections without adding visual weight.
- **Interactive Depth:** Hover states should not "lift" an element with a shadow. Instead, they should shift the background color to a slightly deeper cream or brighten the sage primary color by 5%.
- **Selection:** Active states are indicated by a 2px left-hand solid accent in the primary Sage color, rather than a full-container highlight.

## Shapes

The shape language is **Soft (Level 1)**, moving away from the more playful roundedness of the previous iteration to favor a more precise, professional look.

- **Component Radius:** Elements like buttons and inputs use a consistent 0.25rem (4px) radius.
- **Container Radius:** Larger cards and panels use `rounded-lg` (0.5rem / 8px). 
- **The "Pill" Exception:** Chips and badges remain `rounded-xl` to provide a distinct visual contrast from the rectangular functional components, helping users differentiate between metadata and actions.

## Components

### Buttons
Primary buttons use a solid Muted Sage (`#6B8E7D`) with white text. To maintain the airy feel, secondary buttons are "Ghost" style—using only a 1px Sage border and sage text, with no background fill until hovered.

### Input Fields
Inputs are styled with a transparent background and a 1px `border-ethereal`. On focus, the border transitions to a solid 1px Sage and the background shifts slightly to a pure white (`#FFFFFF`) to highlight the active entry area.

### Cards & Containers
Cards for legal research should be "flat." They utilize the 1px ethereal border and no shadow. The internal padding should be generous (24px minimum) to ensure the text has a high "void-to-content" ratio.

### Checkboxes & Radios
These use the Sage primary color for the active state. They are kept small and precise with 2px corners for checkboxes, aligning with the "Soft" shape language.

### Document Viewer
The flagship component. It features extra-wide margins (up to 80px on desktop) and utilizes Source Serif 4. To reinforce the airy aesthetic, it should not be contained in a "box"; it should bleed into the cream background, using only the typography and spacing to define its bounds.