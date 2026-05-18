/**
 * Re-export of `motion/react`'s `domAnimation` feature bundle in its own
 * module so `LazyMotion` can request it via a dynamic import. Keeping the
 * import isolated lets Vite split the feature bundle into a separate
 * chunk instead of including it in the main app bundle.
 */
export { domAnimation as default } from "motion/react";
