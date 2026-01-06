# Frontend Testing Guide

Comprehensive testing setup for the Codex frontend using Vitest and React Testing Library.

## Tech Stack

- **Test Runner**: Vitest 2.1.8 (fast, Vite-native)
- **Component Testing**: React Testing Library 16.1.0
- **User Interactions**: @testing-library/user-event 14.5.2
- **Assertions**: @testing-library/jest-dom 6.6.3
- **DOM Environment**: jsdom 25.0.1
- **API Mocking**: MSW 2.7.0 (installed, not yet configured)

## Quick Start

### Run Tests

```bash
# Run tests in watch mode
npm test

# Run tests once (CI)
npm run test:run

# Run with UI
npm run test:ui

# Run with coverage
npm run test:coverage
```

## Test Structure

### Directory Layout

```
web/
├── src/
│   ├── test/
│   │   ├── setup.ts          # Global test setup
│   │   └── utils.tsx         # Test utilities and custom render
│   ├── store/
│   │   └── authStore.test.ts
│   ├── api/
│   │   └── client.test.ts
│   ├── pages/
│   │   ├── Login.test.tsx
│   │   └── Home.test.tsx
│   └── components/
│       └── layout/
│           └── Sidebar.test.tsx
└── vitest.config.ts
```

### Test Coverage

Current test files:
- ✅ `authStore.test.ts` - Zustand auth store (4 tests)
- ✅ `client.test.ts` - API client and interceptors (5 tests)
- ✅ `Login.test.tsx` - Login component (5 tests)
- ✅ `Home.test.tsx` - Home/Libraries page (7 tests)
- ✅ `Sidebar.test.tsx` - Sidebar navigation (4 tests)

**Total: 25 tests passing**

## Configuration

### Vitest Config ([vitest.config.ts](vitest.config.ts))

```typescript
export default defineConfig({
  plugins: [react(), tsconfigPaths()],
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: './src/test/setup.ts',
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
    },
  },
});
```

### Test Setup ([src/test/setup.ts](src/test/setup.ts))

Includes:
- Automatic cleanup after each test
- Mock implementations for:
  - `window.matchMedia`
  - `IntersectionObserver`
  - `ResizeObserver`
- Jest-DOM matchers
- localStorage clearing

## Writing Tests

### Custom Render Function

Use `renderWithProviders` to render components with all necessary providers:

```typescript
import { renderWithProviders, screen, userEvent } from '@/test/utils';

it('should render component', () => {
  renderWithProviders(<MyComponent />);
  expect(screen.getByText('Hello')).toBeInTheDocument();
});
```

Includes:
- MantineProvider with theme
- QueryClientProvider (TanStack Query)
- BrowserRouter (React Router)

### Testing Patterns

#### 1. Component Rendering

```typescript
it('should render login form', () => {
  renderWithProviders(<Login />);

  expect(screen.getByText('Welcome to Codex')).toBeInTheDocument();
  expect(screen.getByLabelText(/username/i)).toBeInTheDocument();
  expect(screen.getByRole('button', { name: /sign in/i })).toBeInTheDocument();
});
```

#### 2. User Interactions

```typescript
it('should handle form submission', async () => {
  const user = userEvent.setup();
  renderWithProviders(<Login />);

  await user.type(screen.getByLabelText(/username/i), 'testuser');
  await user.type(screen.getByLabelText(/password/i), 'password123');
  await user.click(screen.getByRole('button', { name: /sign in/i }));

  // Assertions...
});
```

#### 3. Async Operations

```typescript
it('should fetch and display data', async () => {
  vi.mocked(librariesApi.getAll).mockResolvedValueOnce(mockData);

  renderWithProviders(<Home />);

  await waitFor(() => {
    expect(screen.getByText('Comics')).toBeInTheDocument();
  });
});
```

#### 4. Testing Zustand Stores

```typescript
it('should update auth state', () => {
  const mockUser = { id: '1', username: 'test' };

  useAuthStore.getState().setAuth(mockUser, 'token');

  const state = useAuthStore.getState();
  expect(state.user).toEqual(mockUser);
  expect(state.isAuthenticated).toBe(true);
});
```

#### 5. Testing API Clients

```typescript
vi.mock('@/api/auth');

it('should call API on submit', async () => {
  vi.mocked(authApi.login).mockResolvedValueOnce(mockResponse);

  // ... render and interact

  await waitFor(() => {
    expect(authApi.login).toHaveBeenCalledWith({
      username: 'testuser',
      password: 'password123',
    });
  });
});
```

#### 6. Testing Mantine Components

Mantine components don't always have semantic roles. Use these strategies:

```typescript
// For loaders (no progressbar role)
const { container } = renderWithProviders(<Component />);
expect(container.querySelector('.mantine-Loader-root')).toBeTruthy();

// For custom attributes
expect(element).toHaveAttribute('data-active', 'true');

// For text content
expect(screen.getByText('Text')).toBeInTheDocument();
```

## Test Examples

### Store Tests

```typescript
describe('authStore', () => {
  beforeEach(() => {
    useAuthStore.setState({
      user: null,
      token: null,
      isAuthenticated: false,
    });
    localStorage.clear();
  });

  it('should set auth state', () => {
    const mockUser = { /* ... */ };
    useAuthStore.getState().setAuth(mockUser, 'token');

    expect(useAuthStore.getState().isAuthenticated).toBe(true);
    expect(localStorage.getItem('jwt_token')).toBe('token');
  });
});
```

### Component Tests

```typescript
describe('Login Component', () => {
  it('should show error on login failure', async () => {
    const user = userEvent.setup();
    vi.mocked(authApi.login).mockRejectedValueOnce({
      error: 'Invalid credentials'
    });

    renderWithProviders(<Login />);

    await user.type(screen.getByLabelText(/username/i), 'wrong');
    await user.type(screen.getByLabelText(/password/i), 'wrong');
    await user.click(screen.getByRole('button', { name: /sign in/i }));

    await waitFor(() => {
      expect(screen.getByText(/invalid credentials/i)).toBeInTheDocument();
    });
  });
});
```

### Integration Tests

```typescript
describe('Home Component', () => {
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
});
```

## Best Practices

### 1. Test User Behavior, Not Implementation

❌ Bad:
```typescript
expect(component.state.count).toBe(5);
```

✅ Good:
```typescript
expect(screen.getByText('Count: 5')).toBeInTheDocument();
```

### 2. Use Semantic Queries

Priority order:
1. `getByRole` - Most accessible
2. `getByLabelText` - Form elements
3. `getByText` - Non-interactive elements
4. `getByTestId` - Last resort

### 3. Clean Up After Tests

```typescript
beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
});
```

### 4. Mock External Dependencies

```typescript
vi.mock('@/api/auth');
vi.mock('@/api/libraries');
```

### 5. Test Loading and Error States

```typescript
it('should show loading state', () => {
  vi.mocked(api.getData).mockImplementationOnce(
    () => new Promise(() => {}) // Never resolves
  );

  renderWithProviders(<Component />);
  expect(screen.getByText(/loading/i)).toBeInTheDocument();
});

it('should show error state', async () => {
  vi.mocked(api.getData).mockRejectedValueOnce(new Error('Failed'));

  renderWithProviders(<Component />);
  await waitFor(() => {
    expect(screen.getByText(/error/i)).toBeInTheDocument();
  });
});
```

### 6. Test Accessibility

```typescript
it('should be keyboard accessible', async () => {
  const user = userEvent.setup();
  renderWithProviders(<Form />);

  await user.tab(); // Focus first input
  await user.keyboard('username');
  await user.tab(); // Focus second input
  await user.keyboard('password');
  await user.keyboard('{Enter}'); // Submit
});
```

## Coverage Reports

Generate coverage reports:

```bash
npm run test:coverage
```

View HTML report:
```bash
open coverage/index.html
```

Coverage is configured to exclude:
- `node_modules/`
- `src/test/`
- `**/*.d.ts`
- Config files
- Mock data

## CI/CD Integration

### GitHub Actions Example

```yaml
- name: Run tests
  run: npm run test:run

- name: Generate coverage
  run: npm run test:coverage

- name: Upload coverage
  uses: codecov/codecov-action@v3
```

## Troubleshooting

### Common Issues

#### 1. `window.matchMedia is not a function`

Already handled in `src/test/setup.ts`. If you see this error, ensure setup file is loaded.

#### 2. `IntersectionObserver is not defined`

Already mocked in setup. For custom behavior:

```typescript
const mockIntersectionObserver = vi.fn();
global.IntersectionObserver = mockIntersectionObserver;
```

#### 3. Async State Updates

Always use `waitFor` or `findBy` queries:

```typescript
// ❌ May fail
expect(screen.getByText('Loaded')).toBeInTheDocument();

// ✅ Correct
await waitFor(() => {
  expect(screen.getByText('Loaded')).toBeInTheDocument();
});

// ✅ Alternative
const element = await screen.findByText('Loaded');
expect(element).toBeInTheDocument();
```

#### 4. Router Errors

Always wrap components that use routing with `renderWithProviders`:

```typescript
// ❌ Will fail if component uses useNavigate, etc.
render(<Component />);

// ✅ Provides BrowserRouter
renderWithProviders(<Component />);
```

## Future Improvements

- [ ] Set up MSW for API mocking
- [ ] Add E2E tests with Playwright
- [ ] Increase coverage to 80%+
- [ ] Add visual regression testing
- [ ] Add performance testing
- [ ] Add accessibility testing with axe

## Resources

- [Vitest Documentation](https://vitest.dev/)
- [React Testing Library](https://testing-library.com/react)
- [Testing Library Best Practices](https://kentcdodds.com/blog/common-mistakes-with-react-testing-library)
- [MSW Documentation](https://mswjs.io/)

---

**Test Stats:**
- Total Tests: 25
- Test Files: 5
- Passing: 25 (100%)
- Duration: ~1.76s
