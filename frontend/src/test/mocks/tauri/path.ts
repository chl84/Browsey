export const resourceDir = async () => '/mock/resources'
export const homeDir = async () => '/mock'

export const join = async (...parts: string[]) =>
  parts
    .filter((part) => part.length > 0)
    .join('/')
    .replace(/\/+/g, '/')
