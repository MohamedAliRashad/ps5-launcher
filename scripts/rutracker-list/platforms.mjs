// RuTracker console forums the collector can read. Each topic title starts with the tag.
export const PLATFORMS = {
  ps5: { forumId: '546', tag: 'PS5' },
  ps4: { forumId: '973', tag: 'PS4' },
};

/** `--platform ps4|ps5` (default ps5) from command-line arguments. */
export function platformFrom(args) {
  const index = args.indexOf('--platform');
  const key = index < 0 ? 'ps5' : (args[index + 1] || '').toLowerCase();
  const platform = PLATFORMS[key];
  if (!platform) throw new Error(`--platform must be one of: ${Object.keys(PLATFORMS).join(', ')}`);
  return { key, ...platform, source: `https://rutracker.net/forum/viewforum.php?f=${platform.forumId}` };
}
