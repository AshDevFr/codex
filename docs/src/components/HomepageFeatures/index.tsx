import type { ReactNode } from "react";
import clsx from "clsx";
import Heading from "@theme/Heading";
import styles from "./styles.module.css";

type FeatureItem = {
  title: string;
  icon: string;
  description: ReactNode;
};

const FeatureList: FeatureItem[] = [
  {
    title: "Multiple Formats",
    icon: "📚",
    description: (
      <>
        Native support for <strong>CBZ</strong>, <strong>CBR</strong>,{" "}
        <strong>EPUB</strong>, and <strong>PDF</strong>. Automatic metadata
        extraction from ComicInfo.xml and OPF files.
      </>
    ),
  },
  {
    title: "OPDS Catalog",
    icon: "📡",
    description: (
      <>
        Full OPDS 1.2 support for streaming to your favorite reading apps.
        Compatible with Panels, Chunky, Moon+ Reader, and more.
      </>
    ),
  },
  {
    title: "Built with Rust",
    icon: "⚡",
    description: (
      <>
        High performance and memory safety. Efficient resource usage makes Codex
        perfect for large collections on modest hardware.
      </>
    ),
  },
  {
    title: "Scalable Architecture",
    icon: "🔧",
    description: (
      <>
        Run with SQLite for homelab setups or PostgreSQL for larger
        deployments. Stateless design enables horizontal scaling.
      </>
    ),
  },
  {
    title: "Reading Progress",
    icon: "📖",
    description: (
      <>
        Track your reading progress across devices. Resume where you left off
        with automatic page synchronization.
      </>
    ),
  },
  {
    title: "Multi-User Support",
    icon: "👥",
    description: (
      <>
        Create accounts with granular permissions. Each user has their own
        reading progress and library access.
      </>
    ),
  },
];

function Feature({ title, icon, description }: FeatureItem) {
  return (
    <div className={clsx("col col--4")}>
      <div className={styles.featureCard}>
        <div className={styles.featureIcon}>{icon}</div>
        <Heading as="h3" className={styles.featureTitle}>
          {title}
        </Heading>
        <p className={styles.featureDescription}>{description}</p>
      </div>
    </div>
  );
}

export default function HomepageFeatures(): ReactNode {
  return (
    <section className={styles.features}>
      <div className="container">
        <div className="row">
          {FeatureList.map((props, idx) => (
            <Feature key={idx} {...props} />
          ))}
        </div>
      </div>
    </section>
  );
}
