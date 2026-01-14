/**
 * Centralized mock data store
 *
 * Creates and manages mock data with proper relationships between
 * libraries, series, and books.
 */

import { faker } from "@faker-js/faker";
import {
	createBook,
	createLibrary,
	createReadProgress,
	createSeries,
	type MockBook,
	type MockLibrary,
	type MockSeries,
} from "./factories";

// Seed faker for consistent data
faker.seed(12345);

const libraryNames = ["Comics", "Manga", "Ebooks", "Graphic Novels"];

// Series names grouped by library type
const seriesNamesByLibrary: Record<string, string[]> = {
	Comics: [
		"Batman: Year One",
		"Batman: The Dark Knight Returns",
		"Spider-Man: Blue",
		"Amazing Spider-Man",
		"X-Men: Dark Phoenix Saga",
		"Uncanny X-Men",
		"Superman: Red Son",
		"All-Star Superman",
		"Wonder Woman",
		"The Flash",
		"Green Lantern",
		"Justice League",
	],
	Manga: [
		"One Piece",
		"Naruto",
		"Bleach",
		"Dragon Ball",
		"Attack on Titan",
		"My Hero Academia",
		"Demon Slayer",
		"Jujutsu Kaisen",
		"Chainsaw Man",
		"Death Note",
		"Fullmetal Alchemist",
		"Hunter x Hunter",
	],
	Ebooks: [
		"The Expanse",
		"Dune",
		"Foundation",
		"Neuromancer",
		"Snow Crash",
		"The Diamond Age",
		"Hyperion",
		"Ender's Game",
	],
	"Graphic Novels": [
		"Saga",
		"The Walking Dead",
		"Sandman",
		"Watchmen",
		"Maus",
		"Persepolis",
		"V for Vendetta",
		"Preacher",
		"Y: The Last Man",
		"Fables",
	],
};

// Create libraries
export let mockLibraries: MockLibrary[] = libraryNames.map((name) =>
	createLibrary({ name }),
);

// Create series with proper library relationships
export let mockSeries: MockSeries[] = [];
for (const library of mockLibraries) {
	const seriesNames = seriesNamesByLibrary[library.name] || [];
	for (const name of seriesNames) {
		const series = createSeries({
			libraryId: library.id,
			name,
		});
		// Add library name for UI convenience (not in API schema)
		series.libraryName = library.name;
		mockSeries.push(series);
	}
}

// Create books with proper series relationships
export let mockBooks: MockBook[] = [];
for (let seriesIndex = 0; seriesIndex < mockSeries.length; seriesIndex++) {
	const series = mockSeries[seriesIndex];
	const bookCount = 8;
	for (let i = 0; i < bookCount; i++) {
		const hasProgress = seriesIndex < 8 && i < 2;
		const book = createBook({
			seriesId: series.id,
			seriesName: series.name,
			libraryId: series.libraryId,
			number: i + 1,
		});

		if (hasProgress) {
			const totalPages = book.pageCount;
			const currentPage = faker.number.int({ min: 1, max: totalPages - 1 });
			book.readProgress = createReadProgress({
				bookId: book.id,
				currentPage,
				totalPages,
				percentage: Math.round((currentPage / totalPages) * 100),
				isCompleted: false,
			});
		}

		mockBooks.push(book);
	}
}

// Update library counts
mockLibraries = mockLibraries.map((library) => {
	const librarySeries = mockSeries.filter((s) => s.libraryId === library.id);
	const libraryBooks = mockBooks.filter((b) =>
		librarySeries.some((s) => s.id === b.seriesId),
	);
	return {
		...library,
		seriesCount: librarySeries.length,
		bookCount: libraryBooks.length,
	};
});

// Helper functions
export const getSeriesByLibrary = (libraryId: string): MockSeries[] =>
	mockSeries.filter((s) => s.libraryId === libraryId);

export const getBooksByLibrary = (libraryId: string): MockBook[] => {
	const librarySeries = getSeriesByLibrary(libraryId);
	return mockBooks.filter((b) =>
		librarySeries.some((s) => s.id === b.seriesId),
	);
};

export const getBooksBySeries = (seriesId: string): MockBook[] =>
	mockBooks.filter((b) => b.seriesId === seriesId);

// Reset function for testing
export const resetMockData = () => {
	faker.seed(12345);

	mockLibraries = libraryNames.map((name) => createLibrary({ name }));

	mockSeries = [];
	for (const library of mockLibraries) {
		const seriesNames = seriesNamesByLibrary[library.name] || [];
		for (const name of seriesNames) {
			const series = createSeries({
				libraryId: library.id,
				name,
			});
			// Add library name for UI convenience (not in API schema)
			series.libraryName = library.name;
			mockSeries.push(series);
		}
	}

	mockBooks = [];
	for (let seriesIndex = 0; seriesIndex < mockSeries.length; seriesIndex++) {
		const series = mockSeries[seriesIndex];
		const bookCount = 8;
		for (let i = 0; i < bookCount; i++) {
			const hasProgress = seriesIndex < 8 && i < 2;
			const book = createBook({
				seriesId: series.id,
				seriesName: series.name,
				libraryId: series.libraryId,
				number: i + 1,
			});

			if (hasProgress) {
				const totalPages = book.pageCount;
				const currentPage = faker.number.int({ min: 1, max: totalPages - 1 });
				book.readProgress = createReadProgress({
					bookId: book.id,
					currentPage,
					totalPages,
					percentage: Math.round((currentPage / totalPages) * 100),
					isCompleted: false,
				});
			}

			mockBooks.push(book);
		}
	}

	mockLibraries = mockLibraries.map((library) => {
		const librarySeries = mockSeries.filter((s) => s.libraryId === library.id);
		const libraryBooks = mockBooks.filter((b) =>
			librarySeries.some((s) => s.id === b.seriesId),
		);
		return {
			...library,
			seriesCount: librarySeries.length,
			bookCount: libraryBooks.length,
		};
	});
};
