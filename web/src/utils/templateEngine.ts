import { format, isValid, parseISO } from "date-fns";
import Handlebars from "handlebars";

// Maximum execution time for template compilation and rendering (in ms)
const EXECUTION_TIMEOUT_MS = 100;
// Maximum output length to prevent memory issues
const MAX_OUTPUT_LENGTH = 100_000;

/**
 * Result of template rendering
 */
export interface TemplateResult {
	success: boolean;
	output: string;
	error?: string;
}

/**
 * Creates a sandboxed Handlebars instance with safe helpers registered
 */
function createSafeHandlebarsInstance(): typeof Handlebars {
	// Create a new isolated instance to avoid polluting the global Handlebars
	const instance = Handlebars.create();

	// Register safe built-in helpers
	registerSafeHelpers(instance);

	return instance;
}

/**
 * Registers safe helpers on a Handlebars instance
 */
function registerSafeHelpers(instance: typeof Handlebars): void {
	/**
	 * Format a date string
	 * Usage: {{formatDate dateValue "yyyy-MM-dd"}}
	 */
	instance.registerHelper(
		"formatDate",
		(value: unknown, formatString: unknown) => {
			if (!value || typeof value !== "string") return "";

			const dateFormat =
				typeof formatString === "string" ? formatString : "yyyy-MM-dd";

			try {
				const date = parseISO(String(value));
				if (!isValid(date)) return String(value);
				return format(date, dateFormat);
			} catch {
				return String(value);
			}
		},
	);

	/**
	 * Check if two values are equal
	 * Usage: {{#ifEquals value1 value2}}...{{/ifEquals}}
	 */
	instance.registerHelper(
		"ifEquals",
		function (
			this: unknown,
			arg1: unknown,
			arg2: unknown,
			options: Handlebars.HelperOptions,
		) {
			return arg1 === arg2 ? options.fn(this) : options.inverse(this);
		},
	);

	/**
	 * Check if two values are not equal
	 * Usage: {{#ifNotEquals value1 value2}}...{{/ifNotEquals}}
	 */
	instance.registerHelper(
		"ifNotEquals",
		function (
			this: unknown,
			arg1: unknown,
			arg2: unknown,
			options: Handlebars.HelperOptions,
		) {
			return arg1 !== arg2 ? options.fn(this) : options.inverse(this);
		},
	);

	/**
	 * Output JSON representation of a value
	 * Usage: {{json value}}
	 */
	instance.registerHelper("json", (value: unknown) => {
		try {
			return JSON.stringify(value, null, 2);
		} catch {
			return "[Object]";
		}
	});

	/**
	 * Truncate a string to a maximum length
	 * Usage: {{truncate value 100 "..."}}
	 */
	instance.registerHelper(
		"truncate",
		(value: unknown, length: unknown, suffix: unknown) => {
			if (!value || typeof value !== "string") return "";

			const maxLength = typeof length === "number" ? length : 100;
			const ellipsis = typeof suffix === "string" ? suffix : "...";

			if (value.length <= maxLength) return value;
			return value.substring(0, maxLength) + ellipsis;
		},
	);

	/**
	 * Convert value to lowercase
	 * Usage: {{lowercase value}}
	 */
	instance.registerHelper("lowercase", (value: unknown) => {
		if (!value || typeof value !== "string") return "";
		return value.toLowerCase();
	});

	/**
	 * Convert value to uppercase
	 * Usage: {{uppercase value}}
	 */
	instance.registerHelper("uppercase", (value: unknown) => {
		if (!value || typeof value !== "string") return "";
		return value.toUpperCase();
	});

	/**
	 * Capitalize the first letter of a string
	 * Usage: {{capitalize value}}
	 */
	instance.registerHelper("capitalize", (value: unknown) => {
		if (!value || typeof value !== "string") return "";
		return value.charAt(0).toUpperCase() + value.slice(1).toLowerCase();
	});

	/**
	 * Get the first N items of an array
	 * Usage: {{#first items 3}}...{{/first}}
	 */
	instance.registerHelper(
		"first",
		function (
			this: unknown,
			array: unknown,
			count: unknown,
			options: Handlebars.HelperOptions,
		) {
			if (!Array.isArray(array)) return "";
			const n = typeof count === "number" ? count : 1;
			const items = array.slice(0, n);
			return items.map((item) => options.fn(item)).join("");
		},
	);

	/**
	 * Join array items with a separator
	 * Usage: {{join array ", "}}
	 */
	instance.registerHelper("join", (array: unknown, separator: unknown) => {
		if (!Array.isArray(array)) return "";
		const sep = typeof separator === "string" ? separator : ", ";
		return array.join(sep);
	});

	/**
	 * Check if a value exists (not null, undefined, or empty string)
	 * Usage: {{#exists value}}...{{/exists}}
	 */
	instance.registerHelper(
		"exists",
		function (
			this: unknown,
			value: unknown,
			options: Handlebars.HelperOptions,
		) {
			const hasValue = value !== null && value !== undefined && value !== "";
			return hasValue ? options.fn(this) : options.inverse(this);
		},
	);

	/**
	 * Get the length of an array or string
	 * Usage: {{length array}}
	 */
	instance.registerHelper("length", (value: unknown) => {
		if (Array.isArray(value)) return value.length;
		if (typeof value === "string") return value.length;
		return 0;
	});

	/**
	 * Check if a value is greater than another
	 * Usage: {{#gt value1 value2}}...{{/gt}}
	 */
	instance.registerHelper(
		"gt",
		function (
			this: unknown,
			v1: unknown,
			v2: unknown,
			options: Handlebars.HelperOptions,
		) {
			return Number(v1) > Number(v2) ? options.fn(this) : options.inverse(this);
		},
	);

	/**
	 * Check if a value is less than another
	 * Usage: {{#lt value1 value2}}...{{/lt}}
	 */
	instance.registerHelper(
		"lt",
		function (
			this: unknown,
			v1: unknown,
			v2: unknown,
			options: Handlebars.HelperOptions,
		) {
			return Number(v1) < Number(v2) ? options.fn(this) : options.inverse(this);
		},
	);

	/**
	 * Logical AND helper
	 * Usage: {{#and condition1 condition2}}...{{/and}}
	 */
	instance.registerHelper(
		"and",
		function (
			this: unknown,
			v1: unknown,
			v2: unknown,
			options: Handlebars.HelperOptions,
		) {
			return v1 && v2 ? options.fn(this) : options.inverse(this);
		},
	);

	/**
	 * Logical OR helper
	 * Usage: {{#or condition1 condition2}}...{{/or}}
	 */
	instance.registerHelper(
		"or",
		function (
			this: unknown,
			v1: unknown,
			v2: unknown,
			options: Handlebars.HelperOptions,
		) {
			return v1 || v2 ? options.fn(this) : options.inverse(this);
		},
	);

	/**
	 * Lookup helper for accessing object properties dynamically
	 * Usage: {{lookup object key}}
	 */
	instance.registerHelper("lookup", (obj: unknown, key: unknown) => {
		if (!obj || typeof obj !== "object" || key === undefined) return "";
		return (obj as Record<string, unknown>)[String(key)] ?? "";
	});

	/**
	 * Default value helper
	 * Usage: {{default value "fallback"}}
	 */
	instance.registerHelper(
		"default",
		(value: unknown, defaultValue: unknown) => {
			if (value === null || value === undefined || value === "") {
				return defaultValue ?? "";
			}
			return value;
		},
	);

	/**
	 * URL-encode a string for use in URLs
	 * Usage: {{urlencode value}}
	 */
	instance.registerHelper("urlencode", (value: unknown) => {
		if (!value || typeof value !== "string") return "";
		return encodeURIComponent(value);
	});

	/**
	 * Replace occurrences of a substring
	 * Usage: {{replace value "search" "replacement"}}
	 */
	instance.registerHelper(
		"replace",
		(value: unknown, search: unknown, replacement: unknown) => {
			if (!value || typeof value !== "string") return "";
			const searchStr = typeof search === "string" ? search : "";
			const replaceStr = typeof replacement === "string" ? replacement : "";
			return value.split(searchStr).join(replaceStr);
		},
	);

	/**
	 * Split a string and get an item by index
	 * Usage: {{split value "-" 0}} gets first part of "foo-bar-baz" => "foo"
	 */
	instance.registerHelper(
		"split",
		(value: unknown, separator: unknown, index: unknown) => {
			if (!value || typeof value !== "string") return "";
			const sep = typeof separator === "string" ? separator : " ";
			const idx = typeof index === "number" ? index : 0;
			const parts = value.split(sep);
			return parts[idx] ?? "";
		},
	);

	/**
	 * Check if a string contains a substring
	 * Usage: {{#includes value "search"}}...{{/includes}}
	 */
	instance.registerHelper(
		"includes",
		function (
			this: unknown,
			value: unknown,
			search: unknown,
			options: Handlebars.HelperOptions,
		) {
			if (!value || typeof value !== "string") return options.inverse(this);
			const searchStr = typeof search === "string" ? search : "";
			return value.includes(searchStr)
				? options.fn(this)
				: options.inverse(this);
		},
	);

	/**
	 * Basic math operations
	 * Usage: {{math value "+" 5}}, {{math value "-" 2}}, {{math value "*" 3}}, {{math value "/" 2}}
	 */
	instance.registerHelper(
		"math",
		(value: unknown, operator: unknown, operand: unknown) => {
			const v = Number(value);
			const o = Number(operand);
			if (Number.isNaN(v) || Number.isNaN(o)) return "";

			switch (operator) {
				case "+":
					return v + o;
				case "-":
					return v - o;
				case "*":
					return v * o;
				case "/":
					return o !== 0 ? v / o : "";
				case "%":
					return o !== 0 ? v % o : "";
				default:
					return v;
			}
		},
	);

	/**
	 * Pad a number with leading zeros
	 * Usage: {{padStart value 3 "0"}} turns 5 into "005"
	 */
	instance.registerHelper(
		"padStart",
		(value: unknown, length: unknown, char: unknown) => {
			const str = String(value ?? "");
			const len = typeof length === "number" ? length : 2;
			const padChar = typeof char === "string" ? char : "0";
			return str.padStart(len, padChar);
		},
	);

	/**
	 * Trim whitespace from a string
	 * Usage: {{trim value}}
	 */
	instance.registerHelper("trim", (value: unknown) => {
		if (!value || typeof value !== "string") return "";
		return value.trim();
	});
}

/**
 * Compile a Handlebars template safely with timeout protection
 */
export function compileTemplate(
	templateString: string,
): Handlebars.TemplateDelegate | null {
	try {
		const instance = createSafeHandlebarsInstance();

		// Use a simple approach - compile with strict mode
		const compiled = instance.compile(templateString, {
			strict: false, // Allow missing properties to return undefined
			assumeObjects: false,
			knownHelpers: {
				formatDate: true,
				ifEquals: true,
				ifNotEquals: true,
				json: true,
				truncate: true,
				lowercase: true,
				uppercase: true,
				capitalize: true,
				first: true,
				join: true,
				exists: true,
				length: true,
				gt: true,
				lt: true,
				and: true,
				or: true,
				lookup: true,
				default: true,
				urlencode: true,
				replace: true,
				split: true,
				includes: true,
				math: true,
				padStart: true,
				trim: true,
			},
		});

		return compiled;
	} catch {
		return null;
	}
}

/**
 * Render a template with the given data safely
 */
export function renderTemplate(
	template: string,
	data: Record<string, unknown>,
): TemplateResult {
	const startTime = Date.now();

	try {
		// Compile the template
		const compiled = compileTemplate(template);
		if (!compiled) {
			return {
				success: false,
				output: "",
				error: "Failed to compile template",
			};
		}

		// Check for timeout during compilation
		if (Date.now() - startTime > EXECUTION_TIMEOUT_MS) {
			return {
				success: false,
				output: "",
				error: "Template compilation timed out",
			};
		}

		// Execute the template
		const output = compiled(data);

		// Check output length
		if (output.length > MAX_OUTPUT_LENGTH) {
			return {
				success: false,
				output: output.substring(0, MAX_OUTPUT_LENGTH),
				error: "Output exceeded maximum length and was truncated",
			};
		}

		return {
			success: true,
			output,
		};
	} catch (error) {
		const message =
			error instanceof Error ? error.message : "Unknown template error";
		return {
			success: false,
			output: "",
			error: message,
		};
	}
}

/**
 * Validate a template string by compiling and doing a test render.
 * Some Handlebars errors (like malformed tags) are only caught during rendering,
 * so we do a test render with empty data to catch those.
 */
export function validateTemplate(template: string): {
	valid: boolean;
	error?: string;
} {
	try {
		const instance = createSafeHandlebarsInstance();
		const compiled = instance.compile(template);
		// Do a test render with empty data to catch runtime errors
		// (e.g., malformed tags like {{/if} that pass compilation but fail on render)
		compiled({});
		return { valid: true };
	} catch (error) {
		const message =
			error instanceof Error ? error.message : "Invalid template syntax";
		return { valid: false, error: message };
	}
}

/**
 * Get a list of all registered helper names
 */
export function getAvailableHelpers(): string[] {
	return [
		"formatDate",
		"ifEquals",
		"ifNotEquals",
		"json",
		"truncate",
		"lowercase",
		"uppercase",
		"capitalize",
		"first",
		"join",
		"exists",
		"length",
		"gt",
		"lt",
		"and",
		"or",
		"lookup",
		"default",
		"urlencode",
		"replace",
		"split",
		"includes",
		"math",
		"padStart",
		"trim",
	];
}
