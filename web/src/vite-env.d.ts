/// <reference types="vite/client" />

interface ImportMetaEnv {
	readonly VITE_MOCK_API: string;
	readonly VITE_MOCK_SETUP_REQUIRED: string;
	readonly VITE_API_URL: string;
}

interface ImportMeta {
	readonly env: ImportMetaEnv;
}
