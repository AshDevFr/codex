import { describe, it, expect, vi, beforeEach } from 'vitest';
import { screen, waitFor } from '@testing-library/react';
import { renderWithProviders, userEvent } from '@/test/utils';
import { Home } from './Home';
import { librariesApi } from '@/api/libraries';
import type { Library } from '@/types/api';

vi.mock('@/api/libraries');

const mockLibraries: Library[] = [
  {
    id: '1',
    name: 'Comics',
    path: '/data/comics',
    scan_mode: 'AUTO',
    created_at: '2024-01-01T00:00:00Z',
    updated_at: '2024-01-01T00:00:00Z',
    book_count: 150,
    series_count: 25,
    last_scan_at: '2024-01-06T00:00:00Z',
  },
  {
    id: '2',
    name: 'Manga',
    path: '/data/manga',
    scan_mode: 'MANUAL',
    created_at: '2024-01-01T00:00:00Z',
    updated_at: '2024-01-01T00:00:00Z',
    book_count: 200,
    series_count: 30,
  },
];

describe('Home Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should show loading state', async () => {
    vi.mocked(librariesApi.getAll).mockImplementationOnce(
      () => new Promise(() => {}) // Never resolves
    );

    const { container } = renderWithProviders(<Home />);

    // Mantine Loader doesn't have progressbar role, check for the loader element
    await waitFor(() => {
      expect(container.querySelector('.mantine-Loader-root')).toBeTruthy();
    });
  });

  it('should render library grid', async () => {
    vi.mocked(librariesApi.getAll).mockResolvedValueOnce(mockLibraries);

    renderWithProviders(<Home />);

    await waitFor(() => {
      expect(screen.getByText('Comics')).toBeInTheDocument();
      expect(screen.getByText('Manga')).toBeInTheDocument();
    });

    expect(screen.getByText('150 books')).toBeInTheDocument();
    expect(screen.getByText('25 series')).toBeInTheDocument();
    expect(screen.getByText('200 books')).toBeInTheDocument();
    expect(screen.getByText('30 series')).toBeInTheDocument();
  });

  it('should show empty state when no libraries', async () => {
    vi.mocked(librariesApi.getAll).mockResolvedValueOnce([]);

    renderWithProviders(<Home />);

    await waitFor(() => {
      expect(screen.getByText('No libraries found')).toBeInTheDocument();
    });

    expect(
      screen.getByText('Get started by adding your first library')
    ).toBeInTheDocument();
  });

  it('should handle library scan', async () => {
    const user = userEvent.setup();
    vi.mocked(librariesApi.getAll).mockResolvedValue(mockLibraries);
    vi.mocked(librariesApi.scan).mockResolvedValueOnce(undefined);

    renderWithProviders(<Home />);

    await waitFor(() => {
      expect(screen.getByText('Comics')).toBeInTheDocument();
    });

    const scanButtons = screen.getAllByText('Scan Library');
    await user.click(scanButtons[0]);

    await waitFor(() => {
      expect(librariesApi.scan).toHaveBeenCalledWith('1');
    });
  });

  it('should display scan mode badges', async () => {
    vi.mocked(librariesApi.getAll).mockResolvedValueOnce(mockLibraries);

    renderWithProviders(<Home />);

    await waitFor(() => {
      expect(screen.getByText('AUTO')).toBeInTheDocument();
      expect(screen.getByText('MANUAL')).toBeInTheDocument();
    });
  });

  it('should display last scan timestamp', async () => {
    vi.mocked(librariesApi.getAll).mockResolvedValueOnce(mockLibraries);

    renderWithProviders(<Home />);

    await waitFor(() => {
      expect(screen.getByText(/Last scan:/)).toBeInTheDocument();
    });
  });

  it('should show Add Library button', async () => {
    vi.mocked(librariesApi.getAll).mockResolvedValueOnce(mockLibraries);

    renderWithProviders(<Home />);

    await waitFor(() => {
      const addButtons = screen.getAllByText('Add Library');
      expect(addButtons.length).toBeGreaterThan(0);
    });
  });
});
