import httpRequest from './request';

/**
 * 从旧单目录读取文件，拆分覆盖到 project_models / project_infra 后执行初始化导入
 * @param {Object} payload
 * @param {string} payload.sourceDir
 * @returns {Promise<Object>} 导入结果
 */
export async function importProjectFromFiles(payload) {
  const response = await httpRequest.post('/project/import', {
    source_dir: payload?.sourceDir,
  });
  return response;
}
