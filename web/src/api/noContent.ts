/**
 * Normalise a response whose resource may legitimately not exist into `T | null`.
 *
 * Endpoints that model "no value yet" (reading progress on an unstarted book,
 * an unrated series, a plugin that has never run) answer `204 No Content`
 * rather than `200` with a JSON `null` body. That keeps their success body a
 * single concrete type in the OpenAPI document: a nullable body has to be
 * described as a union with `null`, which strict client generators skip
 * outright, producing a client that compiles and cannot see the data.
 *
 * Axios leaves `data` as the empty string for a body-less response, so callers
 * need this to get a real `null` back.
 */
export function noContentAsNull<T>(response: {
  status: number;
  data: T | null | "";
}): T | null {
  if (
    response.status === 204 ||
    response.data === "" ||
    response.data == null
  ) {
    return null;
  }
  return response.data;
}
