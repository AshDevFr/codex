import type { NavigateFunction } from 'react-router-dom';

/**
 * Navigation service to allow navigation outside React components
 * (e.g., in axios interceptors)
 */
class NavigationService {
  private navigate: NavigateFunction | null = null;

  setNavigate(navigateFunction: NavigateFunction) {
    this.navigate = navigateFunction;
  }

  navigateTo(path: string) {
    if (this.navigate) {
      this.navigate(path);
    } else {
      // Fallback to window.location if navigate isn't set yet
      console.warn('Navigate function not set, falling back to window.location');
      window.location.href = path;
    }
  }
}

export const navigationService = new NavigationService();

