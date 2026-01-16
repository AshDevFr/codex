/**
 * Example templates for custom metadata display
 *
 * These pre-built templates provide common patterns for displaying
 * custom metadata on series detail pages. Users can select one as
 * a starting point and customize it for their needs.
 */

import { validateTemplate } from "@/utils/templateEngine";

/**
 * An example template with metadata about the template itself
 */
export interface ExampleTemplate {
	/** Unique identifier for the template */
	id: string;
	/** Display name of the template */
	name: string;
	/** Brief description of what this template does */
	description: string;
	/** The Handlebars template content */
	template: string;
	/** Example data that works well with this template */
	sampleData: Record<string, unknown>;
	/** Tags for categorization */
	tags: string[];
}

/**
 * Default template - Simple key-value list
 */
const defaultTemplate: ExampleTemplate = {
	id: "default",
	name: "Simple List",
	description:
		"Displays all custom metadata fields as a simple bullet list with bold keys.",
	template: `{{#if custom_metadata}}
## Additional Information

{{#each custom_metadata}}
- **{{@key}}**: {{this}}
{{/each}}
{{/if}}`,
	sampleData: {
		source: "Scanned from physical copy",
		edition: "First Edition",
		notes: "Good condition",
	},
	tags: ["basic", "default"],
};

/**
 * Reading list template - Track reading status and progress
 */
const readingListTemplate: ExampleTemplate = {
	id: "reading-list",
	name: "Reading List",
	description:
		"Track reading status, priority, and personal ratings for your library.",
	template: `{{#if custom_metadata}}
## Reading Info

{{#if custom_metadata.status}}
**Status:** {{custom_metadata.status}}
{{/if}}

{{#if custom_metadata.priority}}
**Priority:** {{custom_metadata.priority}}/10
{{/if}}

{{#if custom_metadata.rating}}
**My Rating:** {{custom_metadata.rating}}/10
{{/if}}

{{#if custom_metadata.started_date}}
**Started:** {{formatDate custom_metadata.started_date "MMM d, yyyy"}}
{{/if}}

{{#if custom_metadata.completed_date}}
**Completed:** {{formatDate custom_metadata.completed_date "MMM d, yyyy"}}
{{/if}}

{{#if custom_metadata.notes}}
### Notes
{{custom_metadata.notes}}
{{/if}}
{{/if}}`,
	sampleData: {
		status: "In Progress",
		priority: 8,
		rating: 9,
		started_date: "2024-01-15",
		completed_date: null,
		notes: "Really enjoying this series! The art style is amazing.",
	},
	tags: ["reading", "tracking", "ratings"],
};

/**
 * External links template - Link to external databases and resources
 */
const externalLinksTemplate: ExampleTemplate = {
	id: "external-links",
	name: "External Links",
	description:
		"Display links to external databases like MyAnimeList, AniList, ComicVine, etc.",
	template: `{{#if custom_metadata}}
{{#if custom_metadata.links}}
## External Links

{{#each custom_metadata.links}}
- [{{this.name}}]({{this.url}})
{{/each}}
{{/if}}

{{#if custom_metadata.ids}}
### Database IDs
{{#each custom_metadata.ids}}
- **{{@key}}**: \`{{this}}\`
{{/each}}
{{/if}}
{{/if}}`,
	sampleData: {
		links: [
			{ name: "MyAnimeList", url: "https://myanimelist.net/manga/1" },
			{ name: "AniList", url: "https://anilist.co/manga/1" },
			{ name: "MangaUpdates", url: "https://mangaupdates.com/series/abc123" },
		],
		ids: {
			mal_id: "12345",
			anilist_id: "67890",
			isbn: "978-0-123456-78-9",
		},
	},
	tags: ["links", "external", "databases"],
};

/**
 * Personal notes template - Detailed notes and annotations
 */
const personalNotesTemplate: ExampleTemplate = {
	id: "personal-notes",
	name: "Personal Notes",
	description:
		"Store detailed personal notes, reviews, and annotations for your collection.",
	template: `{{#if custom_metadata}}
{{#if custom_metadata.review}}
## My Review

{{custom_metadata.review}}

{{#if custom_metadata.reviewed_date}}
*Reviewed on {{formatDate custom_metadata.reviewed_date "MMMM d, yyyy"}}*
{{/if}}
{{/if}}

{{#if custom_metadata.highlights}}
### Highlights
{{#each custom_metadata.highlights}}
- {{this}}
{{/each}}
{{/if}}

{{#if custom_metadata.tags}}
**Tags:** {{join custom_metadata.tags ", "}}
{{/if}}
{{/if}}`,
	sampleData: {
		review:
			"An absolute masterpiece of storytelling. The character development is exceptional, and the plot twists kept me engaged throughout.",
		reviewed_date: "2024-06-15",
		highlights: [
			"Chapter 5 has an amazing plot twist",
			"The art in volume 3 is stunning",
			"Great character arc for the protagonist",
		],
		tags: ["favorite", "must-read", "action", "drama"],
	},
	tags: ["notes", "review", "personal"],
};

/**
 * Collection info template - Track physical collection details
 */
const collectionInfoTemplate: ExampleTemplate = {
	id: "collection-info",
	name: "Collection Info",
	description:
		"Track physical collection details like edition, condition, and purchase info.",
	template: `{{#if custom_metadata}}
## Collection Details

{{#if custom_metadata.format}}
**Format:** {{custom_metadata.format}}
{{/if}}

{{#if custom_metadata.edition}}
**Edition:** {{custom_metadata.edition}}
{{/if}}

{{#if custom_metadata.condition}}
**Condition:** {{custom_metadata.condition}}
{{/if}}

{{#if custom_metadata.purchase_date}}
**Purchased:** {{formatDate custom_metadata.purchase_date "MMM d, yyyy"}}
{{/if}}

{{#if custom_metadata.purchase_price}}
**Price:** $\{{custom_metadata.purchase_price}}
{{/if}}

{{#if custom_metadata.location}}
**Location:** {{custom_metadata.location}}
{{/if}}

{{#if custom_metadata.notes}}
### Notes
{{custom_metadata.notes}}
{{/if}}
{{/if}}`,
	sampleData: {
		format: "Hardcover",
		edition: "First Edition",
		condition: "Near Mint",
		purchase_date: "2023-11-20",
		purchase_price: 24.99,
		location: "Shelf 3, Row 2",
		notes: "Signed by author at SDCC 2023",
	},
	tags: ["collection", "physical", "inventory"],
};

/**
 * Comprehensive template - Shows all features combined
 */
const comprehensiveTemplate: ExampleTemplate = {
	id: "comprehensive",
	name: "Comprehensive",
	description:
		"A feature-rich template combining reading status, links, and notes in one view.",
	template: `{{#if custom_metadata}}
{{#and custom_metadata.status custom_metadata.rating}}
## Reading Status

| Status | Rating | Priority |
|--------|--------|----------|
| {{default custom_metadata.status "Not started"}} | {{default custom_metadata.rating "—"}}/10 | {{default custom_metadata.priority "—"}} |

{{/and}}

{{#if custom_metadata.links}}
## Links
{{#each custom_metadata.links}}
- [{{this.name}}]({{this.url}})
{{/each}}
{{/if}}

{{#if custom_metadata.tags}}
**Tags:** {{join custom_metadata.tags " • "}}
{{/if}}

{{#if custom_metadata.notes}}
---
*{{custom_metadata.notes}}*
{{/if}}
{{/if}}`,
	sampleData: {
		status: "Completed",
		rating: 9,
		priority: 10,
		links: [
			{ name: "Official Site", url: "https://example.com" },
			{ name: "Wiki", url: "https://wiki.example.com" },
		],
		tags: ["favorite", "action", "completed"],
		notes:
			"One of the best series in my collection. Highly recommended for fans of the genre.",
	},
	tags: ["comprehensive", "advanced", "full-featured"],
};

/**
 * Minimal template - Just the essentials
 */
const minimalTemplate: ExampleTemplate = {
	id: "minimal",
	name: "Minimal",
	description:
		"A compact template showing only key information in a single line.",
	template: `{{#if custom_metadata}}
{{#if custom_metadata.status}}**{{custom_metadata.status}}**{{/if}}{{#if custom_metadata.rating}} • {{custom_metadata.rating}}/10{{/if}}{{#if custom_metadata.priority}} • Priority: {{custom_metadata.priority}}{{/if}}
{{/if}}`,
	sampleData: {
		status: "Reading",
		rating: 8,
		priority: 5,
	},
	tags: ["minimal", "compact", "simple"],
};

/**
 * Kitchen Sink template - The ultimate showcase
 */
const kitchenSinkTemplate: ExampleTemplate = {
	id: "kitchen-sink",
	name: "🚀 Maximum Overdrive",
	description:
		"I heard you like features, so I put features in your features. This template uses EVERYTHING.",
	template: `{{#if custom_metadata}}
{{#exists custom_metadata.hero}}
# {{uppercase custom_metadata.hero.title}}

{{#gt custom_metadata.hero.power_level 9000}}
⚡ **IT'S OVER 9000!!!** ({{custom_metadata.hero.power_level}} to be exact)
{{else}}
💪 Power Level: {{custom_metadata.hero.power_level}}
{{/gt}}

{{#if custom_metadata.hero.catchphrase}}
> *"{{custom_metadata.hero.catchphrase}}"*
{{/if}}
{{/exists}}

---

{{#and custom_metadata.stats custom_metadata.stats.episodes}}
## 📊 Stats Dashboard

| Metric | Value | Status |
|--------|-------|--------|
| Episodes | {{custom_metadata.stats.episodes}} | {{#gt custom_metadata.stats.episodes 100}}📺 Long Runner{{else}}📺 Standard{{/gt}} |
| Rating | {{custom_metadata.stats.rating}}/10 | {{#gt custom_metadata.stats.rating 8}}🔥 Certified Banger{{else}}{{#gt custom_metadata.stats.rating 5}}👍 Solid{{else}}🤷 Meh{{/gt}}{{/gt}} |
| Rewatches | {{default custom_metadata.stats.rewatches "0"}} | {{#gt custom_metadata.stats.rewatches 2}}🔄 Obsessed{{else}}{{#ifEquals custom_metadata.stats.rewatches 0}}🆕 Fresh Eyes{{else}}🔄 Worth Another{{/ifEquals}}{{/gt}} |
| Hype Level | {{custom_metadata.stats.hype}}% | {{#gt custom_metadata.stats.hype 80}}🚀 MAXIMUM{{else}}📈 Building{{/gt}} |
{{/and}}

{{#exists custom_metadata.timeline}}
## 📅 Timeline

{{#each custom_metadata.timeline}}
- {{#ifEquals this.status "completed"}}✅{{else}}{{#ifEquals this.status "current"}}▶️{{else}}⏳{{/ifEquals}}{{/ifEquals}} **{{this.event}}** — {{formatDate this.date "MMM d, yyyy"}}
{{/each}}
{{/exists}}

{{#if custom_metadata.characters}}
## 👥 Character Ranking

{{#first custom_metadata.characters 3}}
{{#ifEquals @index 0}}🥇{{else}}{{#ifEquals @index 1}}🥈{{else}}🥉{{/ifEquals}}{{/ifEquals}} **{{this.name}}** — *{{this.role}}*
{{#if this.quote}}
> 💬 "{{truncate this.quote 50 "..."}}"
{{/if}}

{{/first}}
{{#gt (length custom_metadata.characters) 3}}
*...and {{length custom_metadata.characters}} more amazing characters!*
{{/gt}}
{{/if}}

{{#exists custom_metadata.genres}}
## 🏷️ Vibes

{{join custom_metadata.genres " • "}}

{{#if custom_metadata.themes}}
**Themes:** {{#each custom_metadata.themes}}{{#if @index}} | {{/if}}{{lowercase this}}{{/each}}
{{/if}}
{{/exists}}

{{#if custom_metadata.technical}}
## 💻 Technical Info

\`\`\`
Resolution: {{custom_metadata.technical.resolution}}
Audio:      {{custom_metadata.technical.audio}}
Subtitles:  {{join custom_metadata.technical.subtitles ", "}}
Source:     {{custom_metadata.technical.source}}
\`\`\`

{{#if custom_metadata.technical.file_id}}
File ID: \`{{custom_metadata.technical.file_id}}\`
{{/if}}
{{/if}}

{{#if custom_metadata.links}}
## 🔗 External Links

{{#each custom_metadata.links}}
- [{{this.name}}]({{this.url}}){{#if this.note}} — *{{this.note}}*{{/if}}
{{/each}}
{{/if}}

{{#exists custom_metadata.verdict}}
---

## 🎬 Final Verdict

{{#gt custom_metadata.verdict.score 9}}
### 🏆 MASTERPIECE
{{else}}{{#gt custom_metadata.verdict.score 7}}
### ⭐ HIGHLY RECOMMENDED
{{else}}{{#gt custom_metadata.verdict.score 5}}
### 👍 WORTH WATCHING
{{else}}
### 🤔 PROCEED WITH CAUTION
{{/gt}}{{/gt}}{{/gt}}

{{custom_metadata.verdict.summary}}

{{#if custom_metadata.verdict.pros}}
**Pros:** {{join custom_metadata.verdict.pros ", "}}
{{/if}}

{{#if custom_metadata.verdict.cons}}
**Cons:** ~~{{join custom_metadata.verdict.cons ", "}}~~ *(minor issues)*
{{/if}}

{{#if custom_metadata.verdict.best_for}}
*Perfect for: {{custom_metadata.verdict.best_for}}*
{{/if}}
{{/exists}}

{{#if custom_metadata.warning}}
> ⚠️ **Content Warning:** {{custom_metadata.warning}}
{{/if}}

---
*Last updated: {{formatDate custom_metadata.last_updated "MMMM d, yyyy 'at' h:mm a"}}*
{{/if}}`,
	sampleData: {
		hero: {
			title: "Attack on Titan",
			power_level: 9001,
			catchphrase:
				"If you win, you live. If you lose, you die. If you don't fight, you can't win!",
		},
		stats: {
			episodes: 87,
			rating: 9.5,
			rewatches: 3,
			hype: 95,
		},
		timeline: [
			{ event: "Started watching", date: "2023-01-15", status: "completed" },
			{ event: "Caught up to S4P1", date: "2023-03-20", status: "completed" },
			{
				event: "Final season premiere",
				date: "2023-11-05",
				status: "completed",
			},
			{ event: "Series finale", date: "2024-01-10", status: "current" },
			{
				event: "Complete rewatch planned",
				date: "2024-06-01",
				status: "pending",
			},
		],
		characters: [
			{
				name: "Levi Ackerman",
				role: "Humanity's Strongest",
				quote:
					"The only thing we're allowed to do is believe that we won't regret the choice we made.",
			},
			{
				name: "Eren Yeager",
				role: "Protagonist",
				quote: "I'll keep moving forward... until I destroy my enemies.",
			},
			{
				name: "Mikasa Ackerman",
				role: "Elite Soldier",
				quote: "This world is cruel, but also very beautiful.",
			},
			{
				name: "Armin Arlert",
				role: "Strategist",
				quote:
					"Someone who can't sacrifice anything can never change anything.",
			},
			{
				name: "Erwin Smith",
				role: "Commander",
				quote: "DEDICATE YOUR HEARTS!",
			},
		],
		genres: [
			"Action",
			"Dark Fantasy",
			"Post-Apocalyptic",
			"Military",
			"Psychological Horror",
		],
		themes: ["FREEDOM", "SURVIVAL", "MORALITY", "SACRIFICE", "CYCLE OF HATRED"],
		technical: {
			resolution: "1920x1080 (Full HD)",
			audio: "Japanese 5.1 / English 5.1",
			subtitles: ["English", "Spanish", "French", "German"],
			source: "Blu-ray Remux",
			file_id: "AOT-S4-BD-001",
		},
		links: [
			{
				name: "MyAnimeList",
				url: "https://myanimelist.net/anime/16498",
				note: "Main page",
			},
			{
				name: "AniList",
				url: "https://anilist.co/anime/16498",
				note: "Track your progress",
			},
			{
				name: "Crunchyroll",
				url: "https://crunchyroll.com/attack-on-titan",
				note: "Official stream",
			},
			{
				name: "Wiki",
				url: "https://attackontitan.fandom.com",
				note: "Spoilers ahead!",
			},
		],
		verdict: {
			score: 9.5,
			summary:
				"A genre-defining masterpiece that redefined what anime could be. From its shocking twists to its complex moral questions, AOT delivers an unforgettable experience that will stay with you long after the final episode.",
			pros: [
				"Mind-blowing plot twists",
				"Deep character development",
				"Epic soundtrack",
				"Beautiful animation",
				"Thought-provoking themes",
			],
			cons: [
				"Can be extremely dark",
				"Long wait between seasons",
				"Divisive ending",
			],
			best_for:
				"Fans of dark fantasy, complex narratives, and emotional roller coasters",
		},
		warning:
			"Contains graphic violence, intense action sequences, and mature themes. Viewer discretion advised.",
		last_updated: "2024-12-28T14:30:00Z",
	},
	tags: ["showcase", "advanced", "everything", "kitchen-sink"],
};

/**
 * All available example templates
 */
export const EXAMPLE_TEMPLATES: ExampleTemplate[] = [
	defaultTemplate,
	readingListTemplate,
	externalLinksTemplate,
	personalNotesTemplate,
	collectionInfoTemplate,
	comprehensiveTemplate,
	minimalTemplate,
	kitchenSinkTemplate,
];

/**
 * Get an example template by ID
 */
export function getTemplateById(id: string): ExampleTemplate | undefined {
	return EXAMPLE_TEMPLATES.find((t) => t.id === id);
}

/**
 * Get the default template
 */
export function getDefaultTemplate(): ExampleTemplate {
	return defaultTemplate;
}

/**
 * Validate all example templates at startup (for testing)
 */
export function validateAllTemplates(): {
	valid: boolean;
	errors: { id: string; error: string }[];
} {
	const errors: { id: string; error: string }[] = [];

	for (const template of EXAMPLE_TEMPLATES) {
		const result = validateTemplate(template.template);
		if (!result.valid) {
			errors.push({ id: template.id, error: result.error || "Unknown error" });
		}
	}

	return {
		valid: errors.length === 0,
		errors,
	};
}
