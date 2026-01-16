/**
 * Mock data factories using Faker.js
 *
 * These factories generate realistic mock data for the API responses.
 * They use the auto-generated types from the OpenAPI schema.
 */

import { faker } from "@faker-js/faker";
import type { components } from "@/types/api.generated";

// Re-export types for convenience
export type UserDto = components["schemas"]["UserDto"];
export type LibraryDto = components["schemas"]["LibraryDto"];
export type SeriesDto = components["schemas"]["SeriesDto"];
export type BookDto = components["schemas"]["BookDto"];

// Extended mock types with additional fields for UI convenience
export type MockLibrary = LibraryDto;
export type MockSeries = SeriesDto & { libraryName?: string };
// MockBook includes libraryId for mock filtering (books are associated with libraries via series)
export type MockBook = BookDto & { libraryId?: string };
export type ReadProgressResponse =
	components["schemas"]["ReadProgressResponse"];
export type MetricsDto = components["schemas"]["MetricsDto"];
export type LibraryMetricsDto = components["schemas"]["LibraryMetricsDto"];
export type TaskMetricsResponse = components["schemas"]["TaskMetricsResponse"];
export type TaskMetricsSummaryDto =
	components["schemas"]["TaskMetricsSummaryDto"];
export type TaskTypeMetricsDto = components["schemas"]["TaskTypeMetricsDto"];
export type QueueHealthMetricsDto =
	components["schemas"]["QueueHealthMetricsDto"];
export type TaskResponse = components["schemas"]["TaskResponse"];
export type TaskStats = components["schemas"]["TaskStats"];
export type SettingDto = components["schemas"]["SettingDto"];
export type SettingHistoryDto = components["schemas"]["SettingHistoryDto"];
export type DuplicateGroup = components["schemas"]["DuplicateGroup"];
export type PaginatedResponse<T> = {
	data: T[];
	page: number;
	pageSize: number;
	total: number;
	totalPages: number;
};

/**
 * Helper to generate consistent UUIDs for related entities.
 * Useful for creating reproducible mock data.
 */
export const seededUuid = (seed: string) => {
	faker.seed(seed.split("").reduce((a, b) => a + b.charCodeAt(0), 0));
	const uuid = faker.string.uuid();
	faker.seed(); // Reset seed
	return uuid;
};

/**
 * User factory - matches UserDto schema
 */
export const createUser = (overrides: Partial<UserDto> = {}): UserDto => ({
	id: faker.string.uuid(),
	username: faker.internet.username(),
	email: faker.internet.email(),
	isAdmin: false,
	isActive: true,
	lastLoginAt: faker.date.recent().toISOString(),
	createdAt: faker.date.past().toISOString(),
	updatedAt: faker.date.recent().toISOString(),
	...overrides,
});

/**
 * Library factory - matches LibraryDto schema
 */
export const createLibrary = (
	overrides: Partial<LibraryDto> = {},
): LibraryDto => {
	const name =
		overrides.name ||
		faker.helpers.arrayElement(["Comics", "Manga", "Ebooks", "Graphic Novels"]);
	return {
		id: faker.string.uuid(),
		name,
		path: `/media/${name.toLowerCase().replace(/\s+/g, "-")}`,
		description: faker.lorem.sentence(),
		isActive: true,
		scanningConfig: null,
		lastScannedAt: faker.date.recent().toISOString(),
		createdAt: faker.date.past().toISOString(),
		updatedAt: faker.date.recent().toISOString(),
		bookCount: faker.number.int({ min: 10, max: 5000 }),
		seriesCount: faker.number.int({ min: 5, max: 500 }),
		allowedFormats: ["CBZ", "CBR", "PDF", "EPUB"],
		excludedPatterns: ".DS_Store\nThumbs.db",
		defaultReadingDirection: "ltr",
		seriesStrategy: "series_volume",
		bookStrategy: "smart",
		numberStrategy: "smart",
		...overrides,
	};
};

/**
 * Series summaries - detailed descriptions for specific series
 */
export const seriesSummaries: Record<string, string> = {
	"Batman: Year One":
		"Frank Miller's groundbreaking retelling of Batman's origin story. Set during Bruce Wayne's first year as the Dark Knight, this noir-influenced masterpiece follows both Batman and Lieutenant James Gordon as they navigate Gotham City's corrupt underbelly. Miller strips away the fantastical elements to deliver a gritty, street-level crime drama that redefined the character for a generation. The story explores themes of justice, corruption, and the moral ambiguity of vigilantism in a city on the brink of collapse.",
	"Batman: The Dark Knight Returns":
		"In a dystopian future where Bruce Wayne has retired from crime-fighting, an aging Dark Knight must don the cape once more to save a Gotham City spiraling into chaos. Frank Miller's seminal work revolutionized the comics industry, presenting a mature, psychological examination of what drives a man to become a symbol of fear. Featuring epic confrontations with Two-Face, the Joker, and Superman himself, this story asks whether Batman can ever truly hang up his cowl.",
	"Spider-Man: Blue":
		"Jeph Loeb and Tim Sale craft an emotional valentine to the early days of Spider-Man. On Valentine's Day, Peter Parker records his memories of Gwen Stacy, the first great love of his life, revisiting their romance amid battles with classic villains like the Green Goblin, the Rhino, and the Lizard. Equal parts superhero action and tender romance, Spider-Man: Blue is a meditation on love, loss, and the people who shape us.",
	"One Piece":
		"Monkey D. Luffy sets sail on an epic adventure to find the legendary treasure known as the One Piece and become King of the Pirates. With his crew of Straw Hat Pirates—each with their own dreams and tragic backstories—Luffy travels across the Grand Line, a treacherous ocean filled with powerful enemies, mysterious islands, and ancient secrets. Eiichiro Oda's magnum opus blends heart-pounding action, laugh-out-loud comedy, and profound themes of freedom, friendship, and pursuing your dreams against all odds.",
	Naruto:
		"Naruto Uzumaki, a young ninja shunned by his village for harboring a powerful demon fox sealed within him, dreams of becoming Hokage—the strongest ninja and leader of his village. Through years of training, fierce battles, and unbreakable bonds of friendship, Naruto transforms from an outcast prankster into a legendary hero. Masashi Kishimoto's epic saga explores themes of perseverance, redemption, and the cycle of hatred, following Naruto and his generation as they reshape the ninja world.",
	"Attack on Titan":
		"In a world where humanity lives behind massive walls to protect themselves from giant humanoid creatures called Titans, young Eren Yeager witnesses the destruction of his home and vows to eradicate every Titan. But as Eren and his comrades in the Survey Corps venture beyond the walls, they discover terrifying truths about the Titans, their world, and themselves. Hajime Isayama's dark fantasy masterpiece subverts expectations at every turn, delivering shocking twists, complex moral dilemmas, and an unflinching examination of war, freedom, and human nature.",
	Saga: "From bestselling writer Brian K. Vaughan and critically acclaimed artist Fiona Staples comes an epic space opera about star-crossed lovers from warring alien races. Alana, a winged soldier, and Marko, a horned deserter, are on the run across the galaxy with their newborn daughter Hazel, hunted by both sides of a never-ending war. Narrated by an adult Hazel looking back on her unconventional childhood, Saga blends science fiction, fantasy, and deeply personal family drama into one of the most acclaimed comics of the modern era.",
	"The Walking Dead":
		"When police officer Rick Grimes awakens from a coma into a world overrun by the undead, he must find his family and navigate the new rules of survival. But the walking dead are only part of the danger—the living can be far more terrifying. Robert Kirkman's landmark series follows Rick and a rotating cast of survivors across years of struggle, loss, and moral compromise, asking what kind of people we become when civilization crumbles and every day is a fight to stay alive.",
	Sandman:
		"After being imprisoned for decades by an occult ritual, Dream of the Endless—also known as Morpheus, the Sandman—escapes and must rebuild his realm of the Dreaming. Neil Gaiman's literary masterpiece weaves mythology, history, horror, and fantasy into a tapestry of interconnected stories spanning millennia. From the gates of Hell to a diner in middle America, from Shakespeare's debut to the end of all things, Sandman explores the nature of stories themselves and the beings who embody our hopes, fears, and dreams.",
	Watchmen:
		"In an alternate 1985 where superheroes exist and America won the Vietnam War, the murder of a former hero sets off a chain of events that could change the world forever. Alan Moore and Dave Gibbons' deconstruction of the superhero genre examines what masked vigilantes would really be like—flawed, neurotic, and sometimes dangerous. Told through multiple perspectives with groundbreaking narrative techniques, Watchmen asks who watches those with power and what price comes with playing god.",
	Dune: "On the desert planet Arrakis, the most valuable substance in the universe—the spice melange—is produced. When young Paul Atreides and his noble family are sent to govern this hostile world, they become pawns in an interstellar power struggle that will transform Paul into something more than human. Frank Herbert's science fiction epic explores ecology, religion, politics, and the dangers of messianic leadership across a richly detailed universe of feuding houses, ancient orders, and desert warriors.",
	Foundation:
		"Mathematician Hari Seldon predicts the fall of the Galactic Empire and a 30,000-year dark age to follow. To shorten this period of barbarism, he establishes the Foundation—a repository of knowledge and science at the edge of the galaxy. Isaac Asimov's visionary series follows the Foundation across centuries as it faces existential crises through the power of science, trade, and political manipulation, asking whether the future can truly be predicted and controlled.",
	"The Expanse":
		"Two hundred years from now, humanity has colonized the solar system but remains divided between Earth, Mars, and the Belt. When a mysterious alien technology is discovered, it threatens to tear these factions apart—or unite them against something far greater. James S.A. Corey's epic series combines hard science fiction with noir detective fiction and political thriller, following a disparate crew thrown together by fate as they become the most important people in the solar system.",
	"My Hero Academia":
		"In a world where 80% of the population has superpowers called Quirks, Izuku Midoriya was born without one. Despite this, he dreams of attending U.A. High School and becoming a hero like his idol, All Might. When fate gives Izuku a chance at power, he enters a world of intense training, fierce rivalries, and deadly villains. Kohei Horikoshi's love letter to superhero comics and shonen manga follows Izuku and his classmates as they learn what it truly means to be a hero in a society built on superpowers.",
	"Demon Slayer":
		"After his family is slaughtered by demons and his sister Nezuko is transformed into one, Tanjiro Kamado begins training as a demon slayer to find a cure and avenge his family. Set in Taisho-era Japan, Koyoharu Gotouge's beautifully illustrated series follows Tanjiro's journey through a secret world of demon slayers and powerful demons, driven by themes of family bonds, perseverance, and the tragedy of those who become monsters.",
	"Fullmetal Alchemist":
		"Brothers Edward and Alphonse Elric broke the ultimate taboo of alchemy—human transmutation—in an attempt to resurrect their mother. The cost was devastating: Edward lost his arm and leg, while Alphonse lost his entire body, his soul now bound to a suit of armor. Now they search for the Philosopher's Stone to restore what they lost, uncovering a conspiracy that threatens their entire nation. Hiromu Arakawa's acclaimed series blends action, humor, and philosophical depth in an exploration of sacrifice, redemption, and the nature of humanity.",
	"Death Note":
		"When brilliant but bored high school student Light Yagami discovers a supernatural notebook that kills anyone whose name is written in it, he decides to create a utopia by eliminating criminals. But as Light's god complex grows, the world's greatest detective—known only as L—begins hunting him. Tsugumi Ohba and Takeshi Obata's psychological thriller is a cat-and-mouse battle of wits that explores the corrupting nature of absolute power and whether the ends can ever justify the means.",
};

/**
 * Series factory - matches SeriesDto schema with optional mock extensions
 */
export const createSeries = (
	overrides: Partial<MockSeries> = {},
): MockSeries => {
	const publishers: Record<string, string> = {
		"Batman: Year One": "DC Comics",
		"Batman: The Dark Knight Returns": "DC Comics",
		"Spider-Man: Blue": "Marvel Comics",
		"Amazing Spider-Man": "Marvel Comics",
		"X-Men: Dark Phoenix Saga": "Marvel Comics",
		"Uncanny X-Men": "Marvel Comics",
		"Superman: Red Son": "DC Comics",
		"All-Star Superman": "DC Comics",
		"Wonder Woman": "DC Comics",
		"The Flash": "DC Comics",
		"Green Lantern": "DC Comics",
		"Justice League": "DC Comics",
		"One Piece": "Shueisha / Viz Media",
		Naruto: "Shueisha / Viz Media",
		Bleach: "Shueisha / Viz Media",
		"Dragon Ball": "Shueisha / Viz Media",
		"Attack on Titan": "Kodansha",
		"My Hero Academia": "Shueisha / Viz Media",
		"Demon Slayer": "Shueisha / Viz Media",
		"Jujutsu Kaisen": "Shueisha / Viz Media",
		"Chainsaw Man": "Shueisha / Viz Media",
		"Death Note": "Shueisha / Viz Media",
		"Fullmetal Alchemist": "Square Enix / Viz Media",
		"Hunter x Hunter": "Shueisha / Viz Media",
		"The Expanse": "Orbit Books",
		Dune: "Ace Books",
		Foundation: "Gnome Press",
		Neuromancer: "Ace Books",
		"Snow Crash": "Bantam Spectra",
		"The Diamond Age": "Bantam Spectra",
		Hyperion: "Doubleday",
		"Ender's Game": "Tor Books",
		Saga: "Image Comics",
		"The Walking Dead": "Image Comics",
		Sandman: "DC Comics / Vertigo",
		Watchmen: "DC Comics",
		Maus: "Pantheon Books",
		Persepolis: "Pantheon Books",
		"V for Vendetta": "DC Comics / Vertigo",
		Preacher: "DC Comics / Vertigo",
		"Y: The Last Man": "DC Comics / Vertigo",
		Fables: "DC Comics / Vertigo",
	};

	const defaultPublishers = [
		"DC Comics",
		"Marvel Comics",
		"Image Comics",
		"Dark Horse",
		"IDW Publishing",
		"Viz Media",
		"Kodansha",
	];

	const title =
		overrides.title ||
		faker.helpers.arrayElement([
			"Batman: Year One",
			"Spider-Man: Blue",
			"Saga",
			"The Walking Dead",
			"One Piece",
			"Attack on Titan",
			"Sandman",
		]);

	const summary =
		seriesSummaries[title] ||
		`${title} is a compelling series that captivates readers with its intricate plot and memorable characters. ` +
			faker.lorem.paragraphs(2, "\n\n");

	const publisher =
		publishers[title] || faker.helpers.arrayElement(defaultPublishers);

	return {
		id: faker.string.uuid(),
		libraryId: overrides.libraryId || faker.string.uuid(),
		libraryName: overrides.libraryName || "Comics",
		title,
		titleSort: title.toLowerCase().replace(/^the\s+/, ""),
		summary,
		publisher,
		year: faker.number.int({ min: 1980, max: 2024 }),
		bookCount: faker.number.int({ min: 1, max: 100 }),
		path: `/media/comics/${title.replace(/[:\s]+/g, "-")}`,
		selectedCoverSource: "first_book",
		hasCustomCover: false,
		unreadCount: faker.number.int({ min: 0, max: 10 }),
		createdAt: faker.date.past().toISOString(),
		updatedAt: faker.date.recent().toISOString(),
		...overrides,
	};
};

/**
 * Book titles and summaries for specific series volumes/issues
 */
export const bookTitlesAndSummaries: Record<
	string,
	{ title: string; summary: string }[]
> = {
	"Batman: Year One": [
		{
			title: "Who I Am",
			summary:
				"Bruce Wayne returns to Gotham after years abroad, beginning his war on crime as Lieutenant James Gordon arrives in the corrupt city. Both men face the overwhelming darkness of Gotham's underworld as they take their first steps toward becoming legends.",
		},
		{
			title: "War on Crime",
			summary:
				"Batman's early attempts at crime-fighting are clumsy and nearly fatal. Meanwhile, Gordon struggles against corruption in the GCPD while trying to protect his pregnant wife from the city's dangers.",
		},
		{
			title: "Black Dawn",
			summary:
				"As Batman refines his methods and strikes fear into the criminal element, Gordon finds himself caught between his duty and the corrupt cops who want him gone—or dead.",
		},
		{
			title: "Friend in Need",
			summary:
				"Batman and Gordon form an unlikely alliance as both face their darkest hour. Selina Kyle emerges from the shadows, and the stage is set for Gotham's transformation.",
		},
	],
	"One Piece": [
		{
			title: "Romance Dawn",
			summary:
				"Monkey D. Luffy, a young man with rubber powers gained from eating the Gum-Gum Fruit, sets out to become King of the Pirates. His first adventure leads him to free a swordsman named Roronoa Zoro from execution.",
		},
		{
			title: "Buggy the Clown",
			summary:
				"Luffy and Zoro encounter the pirate clown Buggy, who possesses the power to separate his body parts. Meanwhile, the crew gains their navigator, Nami, a skilled thief with her own mysterious goals.",
		},
		{
			title: "Usopp's Pirates",
			summary:
				"On Syrup Village, the Straw Hats meet the legendary liar Usopp and uncover a plot by the sinister Captain Kuro. The battle for the village reveals the true courage beneath Usopp's tall tales.",
		},
		{
			title: "The Black Cat Pirates",
			summary:
				"The showdown with Captain Kuro reaches its climax as Usopp must choose between running away and standing to protect everything he loves. The crew gains their ship, the Going Merry.",
		},
		{
			title: "Sanji's Debt",
			summary:
				"At the floating restaurant Baratie, the Straw Hats encounter the legendary pirate Don Krieg and meet Sanji, a chef with dreams of finding the All Blue, a legendary sea where fish from all four oceans gather.",
		},
		{
			title: "Don Krieg's Armada",
			summary:
				"The battle at Baratie intensifies as Luffy faces Don Krieg while Zoro challenges the world's greatest swordsman, Dracule Mihawk. Both fights will change the crew forever.",
		},
		{
			title: "Arlong Park",
			summary:
				"The truth about Nami's past is revealed as the Straw Hats confront Arlong, a fish-man pirate who has enslaved her village for years. Luffy must free Nami from her lifetime of suffering.",
		},
		{
			title: "I Won't Die",
			summary:
				"The epic battle at Arlong Park reaches its conclusion as Luffy destroys everything that symbolizes Nami's enslavement. The Straw Hat Pirates sail toward the Grand Line, their bonds stronger than ever.",
		},
	],
	"Attack on Titan": [
		{
			title: "To You, 2,000 Years From Now",
			summary:
				"In a world where humanity hides behind massive walls from man-eating Titans, young Eren Yeager dreams of the outside world. When the Colossal Titan breaches Wall Maria, Eren's life changes forever.",
		},
		{
			title: "That Day",
			summary:
				"Eren, Mikasa, and Armin join the military Training Corps, each driven by their own reasons. Eren's burning hatred for Titans fuels his determination, but is rage enough to survive?",
		},
		{
			title: "A Dim Light Amid Despair",
			summary:
				"The cadets face their first real battle as Titans breach Trost District. Amidst the chaos and death, Eren makes a discovery that could change everything—or doom them all.",
		},
		{
			title: "First Battle",
			summary:
				"Eren's mysterious power is revealed as he transforms into a Titan. But can he control this ability, and will humanity accept a monster as their savior?",
		},
		{
			title: "Historia",
			summary:
				"The truth about the walls, the Titans, and the royal family begins to unravel. Historia Reiss must confront her past while Eren learns the terrible secret his father left him.",
		},
		{
			title: "The Basement",
			summary:
				"After years of fighting, Eren finally reaches his father's basement. What he discovers there rewrites everything humanity believed about their world and their enemies.",
		},
		{
			title: "Declaration of War",
			summary:
				"Four years after the truth was revealed, the world has changed. Eren takes matters into his own hands, making a choice that will determine the fate of everyone—friend and enemy alike.",
		},
		{
			title: "The Rumbling",
			summary:
				"The final battle begins as ancient powers awaken and march. With the fate of the world hanging in the balance, former enemies must unite to stop a genocide—or die trying.",
		},
	],
	Saga: [
		{
			title: "Chapter One",
			summary:
				"Alana and Marko, soldiers from opposite sides of an endless galactic war, have done the unthinkable—fallen in love and had a child. Now, as new parents, they must flee across the universe while being hunted by both armies.",
		},
		{
			title: "Chapter Two",
			summary:
				"While freelance killers and vengeful ex-fiancées close in, Alana and Marko find unexpected allies. Their daughter Hazel begins narrating her own extraordinary origin story.",
		},
		{
			title: "Chapter Three",
			summary:
				"The family reaches a lighthouse on a rogue moon where they meet author D. Oswald Heist, whose banned romance novel inspired their forbidden relationship. But their pursuers are closing in.",
		},
		{
			title: "Chapter Four",
			summary:
				"Trapped on the planet Quietus, Alana and Marko must navigate a world of drug addiction and showbiz while raising Hazel. The bounty hunters are right behind them.",
		},
		{
			title: "Chapter Five",
			summary:
				"A time skip reveals Hazel as a young girl attending school in secret. Meanwhile, her parents' relationship strains under the weight of their endless running.",
		},
		{
			title: "Chapter Six",
			summary:
				"The family is torn apart by violence and circumstance. As Marko searches desperately for his daughter, Hazel must survive in a world that fears her very existence.",
		},
		{
			title: "Chapter Seven",
			summary:
				"Reunited but scarred, the family seeks refuge with Marko's parents. New enemies emerge as old ones return, and Hazel begins to understand the true cost of war.",
		},
		{
			title: "Chapter Eight",
			summary:
				"The Stalk, The Will, Prince Robot IV—enemies become allies as the stakes grow ever higher. Hazel is no longer just running; she's learning to fight.",
		},
	],
	"The Walking Dead": [
		{
			title: "Days Gone Bye",
			summary:
				"Rick Grimes wakes from a coma into a nightmare. With Atlanta fallen to the dead, he must find his wife and son while learning the brutal rules of survival in a world that has ended.",
		},
		{
			title: "Miles Behind Us",
			summary:
				"The survivors find temporary sanctuary at a farm, but tensions rise as resources dwindle. Rick begins to understand that the living dead may not be the greatest threat they face.",
		},
		{
			title: "Safety Behind Bars",
			summary:
				"A prison offers hope for permanent shelter, but clearing it of the dead is only the beginning. Inside these walls, the survivors will face threats from the living as dangerous as any walker.",
		},
		{
			title: "The Heart's Desire",
			summary:
				"As the prison community grows, so do the complications. Relationships form and break while an external threat looms—the Governor and his town of Woodbury.",
		},
		{
			title: "The Best Defense",
			summary:
				"The Governor's true nature is revealed in horrifying detail. Rick and his group must escape Woodbury, but the cost will shake them to their core.",
		},
		{
			title: "This Sorrowful Life",
			summary:
				"War with Woodbury erupts as the prison is attacked. In the brutal battle that follows, the survivors lose people they love and the home they built together.",
		},
		{
			title: "The Calm Before",
			summary:
				"After the fall of the prison, the scattered survivors must find each other and a new purpose. Alexandria offers a glimpse of the old world—but is it too good to be true?",
		},
		{
			title: "What We Become",
			summary:
				"Years into the apocalypse, Rick Grimes leads a network of communities rebuilding civilization. But new threats emerge, and the choices Rick makes will define humanity's future.",
		},
	],
	Sandman: [
		{
			title: "Preludes & Nocturnes",
			summary:
				"Dream of the Endless escapes decades of imprisonment to find his realm in ruins and his tools of power scattered across the waking world. His quest to reclaim them leads through hell itself.",
		},
		{
			title: "The Doll's House",
			summary:
				"Rose Walker searches for her missing brother, unknowingly serving as a vortex that threatens the Dreaming. Dream must decide whether to destroy her—and face the consequences of his past cruelty.",
		},
		{
			title: "Dream Country",
			summary:
				"Four tales from the corners of the Dreaming: a writer trapped in a muse's prison, a cat's crusade to reshape reality, Shakespeare's deal with Dream, and a night in the life of an immortal.",
		},
		{
			title: "Season of Mists",
			summary:
				"When Lucifer Morningstar abdicates the throne of Hell and gives Dream the key, every pantheon in existence descends on the Dreaming to claim the empty realm. Dream must choose wisely—or not at all.",
		},
		{
			title: "A Game of You",
			summary:
				"Barbie returns to the land she once ruled in dreams, now threatened by the Cuckoo. As reality and dream blur together, five women from a New York apartment fight to save a world of imagination.",
		},
		{
			title: "Fables & Reflections",
			summary:
				"Stories from across history—an African tribe's encounter with Dream, Emperor Norton's madness, Orpheus's tragedy, and more—reveal the Endless's influence on human civilization.",
		},
		{
			title: "Brief Lives",
			summary:
				"Dream joins his irresponsible sister Delirium to search for their missing brother Destruction. The journey forces Dream to confront old loves, old wounds, and a destiny he cannot escape.",
		},
		{
			title: "The Kindly Ones",
			summary:
				"Dream's past sins catch up with him as the Furies are unleashed. The Dreaming itself is under attack, and Dream must face the ultimate consequence of his nature—and his choices.",
		},
	],
	Dune: [
		{
			title: "Dune",
			summary:
				"Young Paul Atreides arrives on Arrakis, the desert planet that produces the universe's most valuable substance. Betrayal, survival, and transformation await as Paul becomes something more than human—the Kwisatz Haderach.",
		},
		{
			title: "Muad'Dib",
			summary:
				"Paul rises among the Fremen, the fierce desert people of Arrakis. As he masters their ways and his own prescient powers, he sets in motion a jihad that will sweep across the known universe.",
		},
		{
			title: "The Prophet",
			summary:
				"Paul Atreides has become Emperor, but the throne is a trap. Surrounded by enemies and haunted by visions of the future, he must navigate political intrigue while his legend grows beyond his control.",
		},
		{
			title: "Dune Messiah",
			summary:
				"Twelve years after Paul's ascent, his empire spans the universe but his prescience shows only doom. A conspiracy threatens everything he's built, and the only escape may be through betrayal and loss.",
		},
		{
			title: "Children of Dune",
			summary:
				"Paul's children Leto and Ghanima possess powers even greater than their father's. As Arrakis transforms and enemies close in, Leto must make a choice that will change humanity's path for millennia.",
		},
		{
			title: "God Emperor of Dune",
			summary:
				"Three thousand years later, Leto II has become something inhuman to guide humanity down the Golden Path. His iron grip ensures survival but at what cost to his soul—and to free will itself?",
		},
		{
			title: "Heretics of Dune",
			summary:
				"The Bene Gesserit struggle to control a universe transformed by Leto II's death. From the desert comes a new threat—and a young woman named Sheeana who can command the sandworms.",
		},
		{
			title: "Chapterhouse: Dune",
			summary:
				"As enemies from the Scattering return to destroy the Bene Gesserit, the sisterhood must adapt or die. On a new desert world, they plant the seeds of Arrakis's future—and humanity's last hope.",
		},
	],
};

/**
 * Book factory - matches BookDto schema
 * Note: libraryId is an extension for mock filtering (books are associated with libraries via series)
 */
export const createBook = (overrides: Partial<MockBook> = {}): MockBook => {
	const seriesName =
		overrides.seriesName ||
		faker.helpers.arrayElement([
			"Batman: Year One",
			"One Piece",
			"Saga",
			"The Walking Dead",
		]);
	const number = overrides.number ?? faker.number.int({ min: 1, max: 50 });

	// Try to get specific book info for this series and number
	const seriesBooks = bookTitlesAndSummaries[seriesName];
	const bookInfo = seriesBooks?.[number - 1];

	// Generate appropriate title based on series type
	let title: string;
	if (overrides.title) {
		title = overrides.title;
	} else if (bookInfo) {
		title = `${seriesName}: ${bookInfo.title}`;
	} else {
		// Default format varies by content type
		const isVolumeBased = [
			"One Piece",
			"Naruto",
			"Attack on Titan",
			"Bleach",
			"Dragon Ball",
			"My Hero Academia",
			"Demon Slayer",
			"Jujutsu Kaisen",
			"Chainsaw Man",
			"Death Note",
			"Fullmetal Alchemist",
			"Hunter x Hunter",
			"Dune",
			"The Expanse",
			"Foundation",
		].includes(seriesName);
		title = isVolumeBased
			? `${seriesName} Vol. ${number}`
			: `${seriesName} #${number}`;
	}

	const formats = ["cbz", "cbr", "pdf", "epub"];

	// Determine file format based on series type
	let fileFormat: string;
	if (overrides.fileFormat) {
		fileFormat = overrides.fileFormat;
	} else if (
		[
			"Dune",
			"The Expanse",
			"Foundation",
			"Neuromancer",
			"Snow Crash",
			"The Diamond Age",
			"Hyperion",
			"Ender's Game",
		].includes(seriesName)
	) {
		fileFormat = faker.helpers.arrayElement(["epub", "pdf"]);
	} else if (
		[
			"One Piece",
			"Naruto",
			"Attack on Titan",
			"Bleach",
			"Dragon Ball",
			"My Hero Academia",
			"Demon Slayer",
			"Jujutsu Kaisen",
			"Chainsaw Man",
			"Death Note",
			"Fullmetal Alchemist",
			"Hunter x Hunter",
		].includes(seriesName)
	) {
		fileFormat = "cbz"; // Manga typically comes in CBZ
	} else {
		fileFormat = faker.helpers.arrayElement(formats);
	}

	// Page count varies by format
	const pageCount =
		overrides.pageCount ??
		(fileFormat === "epub" || fileFormat === "pdf"
			? faker.number.int({ min: 200, max: 600 })
			: faker.number.int({ min: 20, max: 50 }));

	return {
		id: faker.string.uuid(),
		libraryId: overrides.libraryId || faker.string.uuid(),
		libraryName: overrides.libraryName || "Comics",
		seriesId: overrides.seriesId || faker.string.uuid(),
		seriesName,
		title,
		sortTitle: title.toLowerCase(),
		filePath: `/media/comics/${seriesName.replace(/[:\s]+/g, "-")}/${title.replace(/[:\s#]+/g, "-")}.${fileFormat}`,
		fileFormat,
		fileSize: faker.number.int({ min: 10_000_000, max: 100_000_000 }),
		fileHash: faker.string.alphanumeric(40),
		pageCount,
		number,
		createdAt: faker.date.past().toISOString(),
		updatedAt: faker.date.recent().toISOString(),
		readProgress: null,
		readingDirection: "ltr",
		deleted: false,
		...overrides,
	};
};

/**
 * Read progress factory - matches ReadProgressResponse schema
 */
export const createReadProgress = (
	overrides: Partial<ReadProgressResponse> = {},
): ReadProgressResponse => ({
	id: faker.string.uuid(),
	user_id: faker.string.uuid(),
	book_id: faker.string.uuid(),
	current_page: faker.number.int({ min: 1, max: 30 }),
	completed: false,
	completed_at: null,
	started_at: faker.date.past().toISOString(),
	updated_at: faker.date.recent().toISOString(),
	...overrides,
});

/**
 * Setting factory - matches SettingDto schema
 */
export const createSetting = (
	overrides: Partial<SettingDto> = {},
): SettingDto => ({
	id: faker.string.uuid(),
	key:
		overrides.key ||
		faker.helpers.arrayElement([
			"application.name",
			"scanner.scan_timeout_minutes",
			"auth.registration_enabled",
			"task.poll_interval_seconds",
		]),
	value: overrides.value || faker.word.sample(),
	default_value: overrides.default_value || faker.word.sample(),
	description: faker.lorem.sentence(),
	category:
		overrides.category ||
		faker.helpers.arrayElement([
			"Application",
			"Scanner",
			"Authentication",
			"Task",
		]),
	value_type: "string",
	is_sensitive: false,
	updated_at: faker.date.recent().toISOString(),
	updated_by: faker.string.uuid(),
	version: faker.number.int({ min: 1, max: 10 }),
	...overrides,
});

/**
 * Setting history factory - matches SettingHistoryDto schema
 */
export const createSettingHistory = (
	overrides: Partial<SettingHistoryDto> = {},
): SettingHistoryDto => ({
	id: faker.string.uuid(),
	setting_id: faker.string.uuid(),
	key: overrides.key || "server.name",
	old_value: "Old Value",
	new_value: "New Value",
	changed_at: faker.date.recent().toISOString(),
	changed_by: faker.string.uuid(),
	change_reason: faker.lorem.sentence(),
	ip_address: faker.internet.ip(),
	...overrides,
});

/**
 * Task factory - matches TaskDto schema
 */
export const createTask = (
	overrides: Partial<TaskResponse> = {},
): TaskResponse => {
	const statuses: TaskResponse["status"][] = [
		"pending",
		"processing",
		"completed",
		"failed",
	];
	return {
		id: faker.string.uuid(),
		task_type:
			overrides.task_type ||
			faker.helpers.arrayElement([
				"scan_library",
				"generate_thumbnails",
				"analyze_metadata",
			]),
		status: overrides.status || faker.helpers.arrayElement(statuses),
		priority: faker.number.int({ min: 0, max: 10 }),
		attempts: faker.number.int({ min: 0, max: 3 }),
		max_attempts: 3,
		created_at: faker.date.past().toISOString(),
		scheduled_for: faker.date.recent().toISOString(),
		started_at: faker.date.recent().toISOString(),
		completed_at: null,
		last_error: null,
		library_id: faker.string.uuid(),
		book_id: null,
		series_id: null,
		locked_by: null,
		locked_until: null,
		params: null,
		result: null,
		...overrides,
	};
};

/**
 * Task stats factory - matches TaskStats schema
 */
export const createTaskStats = (
	overrides: Partial<TaskStats> = {},
): TaskStats => ({
	pending: faker.number.int({ min: 0, max: 50 }),
	processing: faker.number.int({ min: 0, max: 10 }),
	completed: faker.number.int({ min: 100, max: 5000 }),
	failed: faker.number.int({ min: 0, max: 20 }),
	stale: faker.number.int({ min: 0, max: 5 }),
	total: faker.number.int({ min: 100, max: 5100 }),
	by_type: {},
	...overrides,
});

/**
 * Inventory metrics factory - matches MetricsDto schema
 */
export const createInventoryMetrics = (
	overrides: Partial<MetricsDto> = {},
): MetricsDto => ({
	library_count: faker.number.int({ min: 1, max: 10 }),
	series_count: faker.number.int({ min: 10, max: 500 }),
	book_count: faker.number.int({ min: 100, max: 10000 }),
	total_book_size: faker.number.int({
		min: 1_000_000_000,
		max: 100_000_000_000,
	}),
	user_count: faker.number.int({ min: 1, max: 50 }),
	database_size: faker.number.int({ min: 10_000_000, max: 500_000_000 }),
	page_count: faker.number.int({ min: 10000, max: 500000 }),
	libraries: [],
	...overrides,
});

/**
 * Library metrics factory - matches LibraryMetricsDto schema
 */
export const createLibraryMetrics = (
	overrides: Partial<LibraryMetricsDto> = {},
): LibraryMetricsDto => ({
	id: faker.string.uuid(),
	name: faker.helpers.arrayElement(["Comics", "Manga", "Ebooks"]),
	series_count: faker.number.int({ min: 5, max: 100 }),
	book_count: faker.number.int({ min: 50, max: 2000 }),
	total_size: faker.number.int({ min: 500_000_000, max: 50_000_000_000 }),
	...overrides,
});

/**
 * Task metrics factory - matches TaskMetricsResponse schema
 */
export const createTaskMetrics = (
	overrides: Partial<TaskMetricsResponse> = {},
): TaskMetricsResponse => ({
	updated_at: faker.date.recent().toISOString(),
	retention: "30",
	summary: {
		total_executed: faker.number.int({ min: 100, max: 10000 }),
		total_succeeded: faker.number.int({ min: 90, max: 9000 }),
		total_failed: faker.number.int({ min: 0, max: 100 }),
		avg_duration_ms: faker.number.float({ min: 100, max: 5000 }),
		avg_queue_wait_ms: faker.number.float({ min: 10, max: 500 }),
		tasks_per_minute: faker.number.float({ min: 0.5, max: 20 }),
	},
	by_type: [],
	queue: {
		pending_count: faker.number.int({ min: 0, max: 50 }),
		processing_count: faker.number.int({ min: 0, max: 5 }),
		stale_count: 0,
		oldest_pending_age_ms: null,
	},
	...overrides,
});

/**
 * Task type metrics factory - matches TaskTypeMetricsDto schema
 */
export const createTaskTypeMetrics = (
	overrides: Partial<TaskTypeMetricsDto> = {},
): TaskTypeMetricsDto => ({
	task_type: overrides.task_type || "scan_library",
	executed: faker.number.int({ min: 10, max: 1000 }),
	succeeded: faker.number.int({ min: 9, max: 950 }),
	failed: faker.number.int({ min: 0, max: 50 }),
	retried: faker.number.int({ min: 0, max: 20 }),
	avg_duration_ms: faker.number.float({ min: 500, max: 10000 }),
	min_duration_ms: faker.number.int({ min: 100, max: 500 }),
	max_duration_ms: faker.number.int({ min: 10000, max: 60000 }),
	p50_duration_ms: faker.number.int({ min: 1000, max: 3000 }),
	p95_duration_ms: faker.number.int({ min: 5000, max: 15000 }),
	avg_queue_wait_ms: faker.number.float({ min: 10, max: 200 }),
	items_processed: faker.number.int({ min: 100, max: 50000 }),
	bytes_processed: faker.number.int({ min: 100_000_000, max: 10_000_000_000 }),
	throughput_per_sec: faker.number.float({ min: 1, max: 100 }),
	error_rate_pct: faker.number.float({ min: 0, max: 10 }),
	last_error: null,
	last_error_at: null,
	...overrides,
});

/**
 * Duplicate group factory - matches DuplicateGroup schema
 */
export const createDuplicateGroup = (
	overrides: Partial<DuplicateGroup> = {},
): DuplicateGroup => ({
	id: faker.string.uuid(),
	file_hash: faker.string.alphanumeric(64),
	duplicate_count: faker.number.int({ min: 2, max: 5 }),
	book_ids: [faker.string.uuid(), faker.string.uuid()],
	created_at: faker.date.past().toISOString(),
	updated_at: faker.date.recent().toISOString(),
	...overrides,
});

/**
 * Paginated response factory
 * Matches the server's PaginatedResponse format
 */
export const createPaginatedResponse = <T>(
	data: T[],
	options: { page?: number; pageSize?: number; total?: number } = {},
): PaginatedResponse<T> => {
	const page = options.page ?? 0;
	const pageSize = options.pageSize ?? 20;
	const total = options.total ?? data.length;
	const totalPages = Math.ceil(total / pageSize);

	return {
		data,
		page,
		pageSize,
		total,
		totalPages,
	};
};

/**
 * Create a list of items
 */
export const createList = <T>(
	factory: (index: number) => T,
	count: number,
): T[] => Array.from({ length: count }, (_, i) => factory(i));
