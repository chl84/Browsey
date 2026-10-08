# Shared UI controls

Reuse these components in feature views and dialogs instead of styling native
controls separately:

- `Checkbox`: bind `checked`, or pass `checked` and handle `change`. Use the
  default slot for the label and the `description` slot for supporting text.
- `Radio`: pass a common, unique `name` for each group, a `value`, and a
  controlled `checked` state. Update the owning state in `on:change`. Native
  inputs retain mutual exclusion, form semantics, and arrow-key navigation.
- `TextField`: use for text, search, and password inputs. Bind `element` instead
  of `this` when the caller needs access to the input. Password visibility must
  only change `type`, not the field's styling or dimensions.
- `Slider` and `ComboBox`: use for ranges and custom option selectors.
  For an action selector, use `resetOnSelect` and a descriptive `ariaLabel`;
  choices dispatch `change` without replacing the action placeholder. Options
  may set `disabled`, which blocks pointer selection and is skipped by arrow keys.
  Use `fixedDropdown` inside scrollable lists so choices escape overflow clipping.
- `ModalShell`: use for shared modal layout, focus handling, and action slots.
- `ProgressBar`: use for progress, or set `role="meter"` for measured values
  such as disk usage. Supply a descriptive `label`/`valueText`; `fillColor` may
  reference a theme token without changing the styling of other bars.

Use the existing theme and density variables from `app.css`. Checkbox and radio
indicators intentionally share size, foreground, border, disabled, and focus
tokens; radios remain circular to distinguish single-choice groups.

Modal buttons use shared global styles: `secondary` for cancellation and
secondary actions, `primary` for highlighted non-destructive actions, and
`danger` for destructive actions such as overwrite, delete, and format. Keep a
safe action as the initial/default action when appropriate.

Buttons and elements with `role="button"` inherit the global `:focus-visible`
ring. Do not remove it unless an equally visible keyboard-focus treatment
replaces it. Controls revealed on hover must also be visible on keyboard focus.
