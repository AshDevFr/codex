import type { ReactNode } from "react";

import Link from "@docusaurus/Link";
import useDocusaurusContext from "@docusaurus/useDocusaurusContext";
import HomepageFeatures from "@site/src/components/HomepageFeatures";
import Heading from "@theme/Heading";
import Layout from "@theme/Layout";

import styles from "./index.module.css";

function HomepageHeader() {
  const { siteConfig } = useDocusaurusContext();
  return (
    <header className={styles.heroBanner}>
      <div className="container">
        <Heading as="h1" className={styles.heroTitle}>
          {siteConfig.title}
        </Heading>
        <p className={styles.heroSubtitle}>{siteConfig.tagline}</p>
        <div className={styles.buttons}>
          <Link className={styles.primaryButton} to="/docs/getting-started">
            Get Started
          </Link>
          <Link className={styles.secondaryButton} to="/docs/deployment/docker">
            Docker Setup
          </Link>
        </div>
        <div className={styles.heroCode}>
          <code>docker pull ghcr.io/codex/codex:latest</code>
        </div>
      </div>
    </header>
  );
}

function QuickLinks(): ReactNode {
  return (
    <section className={styles.quickLinks}>
      <div className="container">
        <div className="row">
          <div className="col col--4">
            <Link to="/docs/deployment/docker" className={styles.quickLink}>
              <span className={styles.quickLinkIcon}>🐳</span>
              <span className={styles.quickLinkText}>Docker Setup</span>
            </Link>
          </div>
          <div className="col col--4">
            <Link to="/docs/configuration" className={styles.quickLink}>
              <span className={styles.quickLinkIcon}>⚙️</span>
              <span className={styles.quickLinkText}>Configuration</span>
            </Link>
          </div>
          <div className="col col--4">
            <Link to="/docs/api" className={styles.quickLink}>
              <span className={styles.quickLinkIcon}>🔌</span>
              <span className={styles.quickLinkText}>API Reference</span>
            </Link>
          </div>
        </div>
      </div>
    </section>
  );
}

export default function Home(): ReactNode {
  const { siteConfig } = useDocusaurusContext();
  return (
    <Layout
      title={`${siteConfig.title} - Digital Library Server`}
      description="A next-generation digital library server for comics, manga, and ebooks built in Rust"
    >
      <HomepageHeader />
      <main>
        <QuickLinks />
        <HomepageFeatures />
      </main>
    </Layout>
  );
}
