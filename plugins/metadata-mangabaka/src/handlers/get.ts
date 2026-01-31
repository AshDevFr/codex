import {
  type MetadataGetParams,
  NotFoundError,
  type PluginSeriesMetadata,
} from "@ashdev/codex-plugin-sdk";
import type { MangaBakaClient } from "../api.js";
import { mapSeriesMetadata } from "../mappers.js";

export async function handleGet(
  params: MetadataGetParams,
  client: MangaBakaClient,
): Promise<PluginSeriesMetadata> {
  const seriesId = Number.parseInt(params.externalId, 10);

  if (Number.isNaN(seriesId)) {
    throw new NotFoundError(`Invalid external ID: ${params.externalId}`);
  }

  const response = await client.getSeries(seriesId);

  return mapSeriesMetadata(response);
}
