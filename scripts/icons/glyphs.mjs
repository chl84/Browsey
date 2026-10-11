// One geometry per symbol. Paint roles are resolved for the surrounding file or
// folder by generate.mjs; paths, stroke widths and placement stay identical.
export const glyphs = {
  font: `
    <path d="M36 101L50 63L64 101M41 87H59" stroke="@detail" stroke-width="3" stroke-linejoin="miter" stroke-linecap="butt" />
    <path d="M72 79C76 73 89 74 89 83V101M89 88H79C68 88 68 102 79 102C84 102 89 98 89 92" stroke="@detail" stroke-width="3" stroke-linejoin="miter" stroke-linecap="butt" />`,
  code: `
    <rect x="28" y="60" width="72" height="48" fill="@panel" stroke="@detail" stroke-width="2" />
    <path d="M48 73L38 84L48 95M80 73L90 84L80 95M68 70L60 98" stroke="@detail" stroke-width="3" stroke-linejoin="miter" stroke-linecap="butt" />`,
  document: `
    <rect x="28" y="60" width="72" height="14" fill="@panel" stroke="@detail" stroke-width="2" />
    <path d="M36 84H92M36 94H92M36 104H76" stroke="@detail" stroke-width="3" stroke-linecap="butt" />`,
  music: `
    <ellipse cx="50" cy="96" rx="9" ry="7" fill="@detail" />
    <rect x="54" y="62" width="4" height="36" fill="@detail" />
    <ellipse cx="75" cy="92" rx="8" ry="6" fill="@detail" />
    <rect x="78" y="62" width="4" height="30" fill="@detail" />
    <rect x="56" y="62" width="26" height="6" fill="@detail" />`,
  picture: `
    <rect x="28" y="60" width="72" height="48" fill="@panel" stroke="@detail" stroke-width="2" />
    <circle cx="77" cy="72" r="8" fill="@sun" stroke="@detail" stroke-width="3" />
    <path d="M31 104L51 82L65 92L75 84L97 104H31Z" fill="@terrain" stroke="@detail" stroke-width="3" stroke-linejoin="miter" />`,
  video: `
    <rect x="28" y="60" width="72" height="48" fill="@panel" stroke="@detail" stroke-width="2" />
    <g fill="@shade">
      <rect x="36" y="62" width="8" height="8" />
      <rect x="48" y="62" width="8" height="8" />
      <rect x="60" y="62" width="8" height="8" />
      <rect x="72" y="62" width="8" height="8" />
      <rect x="84" y="62" width="8" height="8" />
      <rect x="36" y="100" width="8" height="8" />
      <rect x="48" y="100" width="8" height="8" />
      <rect x="60" y="100" width="8" height="8" />
      <rect x="72" y="100" width="8" height="8" />
      <rect x="84" y="100" width="8" height="8" />
    </g>
    <circle cx="64" cy="84" r="18" fill="@detail" stroke="@detail" stroke-width="2" />
    <path d="M58 74V94L74 84L58 74Z" fill="@surface" />`,
  book: `
    <rect x="40" y="56" width="48" height="50" fill="@panel" stroke="@detail" stroke-width="2" />
    <rect x="40" y="56" width="9" height="50" fill="@shade" stroke="@detail" stroke-width="2" />
    <path d="M76 56H84V77L80 73L76 77Z" fill="@surface" stroke="@detail" stroke-width="2" stroke-linejoin="miter" />
    <path d="M56 85H80M56 94H73" stroke="@detail" stroke-width="3" stroke-linecap="butt" />`,
  gear: `
    <g transform="translate(64 82) scale(0.82)">
      <g fill="@detail">
        <rect x="-3" y="-28" width="6" height="10" />
        <rect x="-3" y="18" width="6" height="10" />
        <rect x="-28" y="-3" width="10" height="6" />
        <rect x="18" y="-3" width="10" height="6" />
        <rect x="-3" y="-28" width="6" height="10" transform="rotate(45)" />
        <rect x="-3" y="18" width="6" height="10" transform="rotate(45)" />
        <rect x="-28" y="-3" width="10" height="6" transform="rotate(45)" />
        <rect x="18" y="-3" width="10" height="6" transform="rotate(45)" />
      </g>
      <circle cx="0" cy="0" r="20" fill="@shade" stroke="@detail" stroke-width="2" />
      <circle cx="0" cy="0" r="8" fill="@surface" stroke="@detail" stroke-width="2" />
    </g>`,
  box: `
    <path d="M36 68L44 56H84L92 68Z" fill="@shade" stroke="@detail" stroke-width="2" stroke-linejoin="miter" />
    <rect x="36" y="68" width="56" height="38" fill="@panel" stroke="@detail" stroke-width="2" />
    <path d="M59 56H69V106H59Z" fill="@surface" stroke="@detail" stroke-width="2" />
    <path d="M36 68H92" stroke="@detail" stroke-width="2" />`,
  temporary: `
    <path d="M44 60H84M44 104H84" stroke="@detail" stroke-width="3" />
    <path d="M49 64H79L64 82L79 100H49L64 82Z" fill="@panel" stroke="@detail" stroke-width="3" stroke-linejoin="miter" />
    <path d="M57 70H71L64 78Z M53 97L64 86L75 97Z" fill="@detail" />`,
  backup: `
    <path d="M46 70A22 22 0 1 1 43 90M46 59V73H60M64 70V84L74 90" stroke="@detail" stroke-width="3" stroke-linejoin="miter" stroke-linecap="butt" />`,
  cache: `
    <path d="M68 58L46 86H62L58 106L82 76H66Z" fill="@detail" />`,
  games: `
    <path d="M44 70H84L94 98H80L73 89H55L48 98H34Z" fill="@panel" stroke="@detail" stroke-width="3" stroke-linejoin="miter" />
    <path d="M46 75V87M40 81H52" stroke="@detail" stroke-width="3" />
    <circle cx="78" cy="79" r="3" fill="@detail" />
    <circle cx="86" cy="85" r="3" fill="@detail" />`,
  screenshot: `
    <path d="M52 62H76V72H92V104H36V72H52Z" fill="@panel" stroke="@detail" stroke-width="3" stroke-linejoin="miter" />
    <circle cx="64" cy="88" r="10" stroke="@detail" stroke-width="3" />
    <rect x="80" y="77" width="7" height="4" fill="@detail" />`,
}
