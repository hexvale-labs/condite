// Custom bundles must never replace themselves with upstream's official release.
export const buildArguments = standalone => ['build', ...(standalone ? ['--standalone'] : []), '--disable-updater'];
