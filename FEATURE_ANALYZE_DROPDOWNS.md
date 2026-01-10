# Feature: Analyze Dropdown Menus on Book and Series Cards

## Overview

Added dropdown menus with analyze actions to book and series cards throughout the UI.

## Changes Made

### 1. API Client Updates

#### `web/src/api/books.ts`

- Added `analyze(bookId)` - Force analyze a book
- Added `analyzeUnanalyzed(bookId)` - Analyze only if not already analyzed

#### `web/src/api/series.ts`

- Added `analyze(seriesId)` - Force analyze all books in series
- Added `analyzeUnanalyzed(seriesId)` - Analyze only unanalyzed books in series

### 2. Component Updates

#### `web/src/components/library/BooksSection.tsx`

- Added dropdown menu (three dots icon) to BookCard
- Menu includes "Analyze" action
- Shows notifications on success/error
- Invalidates query cache on success

#### `web/src/components/library/SeriesSection.tsx`

- Added dropdown menu (three dots icon) to SeriesCard
- Menu includes two actions:
  - "Analyze All" - Force analyze all books
  - "Analyze Unanalyzed" - Only analyze unanalyzed books
- Shows notifications on success/error
- Invalidates query cache on success

#### `web/src/components/library/RecommendedSection.tsx`

- Updated BookCard component with same dropdown menu as BooksSection
- Updated SeriesCard component with same dropdown menu as SeriesSection
- Maintains consistency across all card locations

### 3. UI/UX Features

- **Icon**: Uses `IconAnalyze` from Tabler Icons
- **Dropdown Trigger**: Three-dot menu icon (IconDots)
- **Position**: Top-right corner of each card
- **States**:
  - Normal state shows "Analyze" / "Analyze All" / "Analyze Unanalyzed"
  - Loading state shows "Analyzing..."
  - Button disabled during analysis
- **Feedback**:
  - Success notification: "Analysis started" with context-appropriate message
  - Error notification: "Analysis failed" with error details
- **Data Refresh**: Query cache automatically invalidated on success

### 4. Backend Integration

Uses existing backend API endpoints:

- `POST /api/v1/books/{id}/analyze` - Force analyze book
- `POST /api/v1/books/{id}/analyze-unanalyzed` - Conditional book analysis
- `POST /api/v1/series/{id}/analyze` - Force analyze all books in series
- `POST /api/v1/series/{id}/analyze-unanalyzed` - Analyze unanalyzed books in series

## Testing

### Manual Testing Steps

1. **Book Cards**:

   - Navigate to Library → Books tab
   - Click three-dot menu on any book card
   - Click "Analyze"
   - Verify notification appears
   - Check that analysis task is queued (via task queue)

2. **Series Cards**:

   - Navigate to Library → Series tab
   - Click three-dot menu on any series card
   - Test "Analyze All" action
   - Test "Analyze Unanalyzed" action
   - Verify appropriate notifications appear
   - Check that analysis tasks are queued for books

3. **Recommended Section**:
   - Navigate to Home page
   - Test dropdown menus on "Keep Reading" book cards
   - Test dropdown menus on "On Deck" series cards
   - Test dropdown menus on "Recently Added" book cards

### Expected Behavior

- Clicking "Analyze" should queue an analysis task for the book/series
- Notification should appear confirming task queued
- Menu should close after action
- Button should be disabled during pending request
- Error states should show appropriate error messages

## Code Quality

- TypeScript: All type-safe, no type errors
- React: Uses proper hooks (useMutation, useQueryClient)
- State Management: Proper mutation handling with TanStack Query
- Error Handling: Comprehensive error handling with user feedback
- Accessibility: Proper ARIA attributes from Mantine components
- Performance: Efficient query invalidation, no unnecessary re-renders

## Future Enhancements

Potential improvements:

1. Add confirmation dialog for "Analyze All" on series with many books
2. Show progress indicator for analysis tasks
3. Add "Analyze Library" action at library level
4. Add batch selection for analyzing multiple items at once
5. Show analysis status badge on cards (analyzed vs. unanalyzed)
