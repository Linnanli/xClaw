import React, { useState, useEffect } from 'react';
import { Modal, Checkbox, message, Spin, Collapse } from 'antd';
import { apiClient } from '../../api/client';
import type { Role, Permission } from '../../types';

interface AssignPermissionsModalProps {
  visible: boolean;
  role: Role;
  onCancel: () => void;
  onSuccess: () => void;
}

export const AssignPermissionsModal: React.FC<AssignPermissionsModalProps> = ({
  visible,
  role,
  onCancel,
  onSuccess,
}) => {
  const [loading, setLoading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [allPermissions, setAllPermissions] = useState<Record<string, Permission[]>>({});
  const [selectedPermissions, setSelectedPermissions] = useState<string[]>([]);

  useEffect(() => {
    if (visible) {
      loadPermissions();
      loadRolePermissions();
    }
  }, [visible, role.id]);

  const loadPermissions = async () => {
    setLoading(true);
    try {
      const response = await apiClient.get('/permissions');
      setAllPermissions(response.data.permissions || {});
    } catch (error: any) {
      message.error('加载权限列表失败');
    } finally {
      setLoading(false);
    }
  };

  const loadRolePermissions = async () => {
    try {
      const response = await apiClient.get(`/roles/${role.id}`);
      const permissions = response.data.permissions || [];
      setSelectedPermissions(permissions.map((p: Permission) => p.id));
    } catch (error: any) {
      message.error('加载角色权限失败');
    }
  };

  const handleSubmit = async () => {
    setSubmitting(true);
    try {
      await apiClient.post(`/roles/${role.id}/permissions`, {
        permission_ids: selectedPermissions,
      });

      message.success('分配权限成功');
      onSuccess();
    } catch (error: any) {
      message.error(error.response?.data?.error || '分配权限失败');
    } finally {
      setSubmitting(false);
    }
  };

  const handlePermissionChange = (permissionId: string, checked: boolean) => {
    if (checked) {
      setSelectedPermissions([...selectedPermissions, permissionId]);
    } else {
      setSelectedPermissions(selectedPermissions.filter(id => id !== permissionId));
    }
  };

  const handleResourceCheckAll = (resource: string, checked: boolean) => {
    const resourcePermissions = allPermissions[resource] || [];
    const resourcePermissionIds = resourcePermissions.map(p => p.id);

    if (checked) {
      const newSelected = [...selectedPermissions];
      resourcePermissionIds.forEach(id => {
        if (!newSelected.includes(id)) {
          newSelected.push(id);
        }
      });
      setSelectedPermissions(newSelected);
    } else {
      setSelectedPermissions(
        selectedPermissions.filter(id => !resourcePermissionIds.includes(id))
      );
    }
  };

  const isResourceChecked = (resource: string) => {
    const resourcePermissions = allPermissions[resource] || [];
    return resourcePermissions.every(p => selectedPermissions.includes(p.id));
  };

  const isResourceIndeterminate = (resource: string) => {
    const resourcePermissions = allPermissions[resource] || [];
    const checkedCount = resourcePermissions.filter(p => selectedPermissions.includes(p.id)).length;
    return checkedCount > 0 && checkedCount < resourcePermissions.length;
  };

  const resourceNames: Record<string, string> = {
    users: '用户管理',
    roles: '角色管理',
    permissions: '权限管理',
    dlp: 'DLP 管理',
    audit: '审计日志',
  };

  return (
    <Modal
      title={`分配权限 - ${role.name}`}
      open={visible}
      onOk={handleSubmit}
      onCancel={onCancel}
      confirmLoading={submitting}
      okText="保存"
      cancelText="取消"
      width={700}
      destroyOnClose
    >
      {loading ? (
        <div style={{ textAlign: 'center', padding: '40px 0' }}>
          <Spin tip="加载中..." />
        </div>
      ) : (
        <Collapse defaultActiveKey={Object.keys(allPermissions)}>
          {Object.entries(allPermissions).map(([resource, permissions]) => (
            <Collapse.Panel
              key={resource}
              header={
                <Checkbox
                  checked={isResourceChecked(resource)}
                  indeterminate={isResourceIndeterminate(resource)}
                  onChange={(e) => {
                    e.stopPropagation();
                    handleResourceCheckAll(resource, e.target.checked);
                  }}
                  onClick={(e) => e.stopPropagation()}
                >
                  {resourceNames[resource] || resource} ({permissions.length} 个权限)
                </Checkbox>
              }
            >
              <div style={{ paddingLeft: '24px' }}>
                {permissions.map((permission) => (
                  <div key={permission.id} style={{ marginBottom: '8px' }}>
                    <Checkbox
                      checked={selectedPermissions.includes(permission.id)}
                      onChange={(e) => handlePermissionChange(permission.id, e.target.checked)}
                    >
                      <span style={{ fontWeight: 500 }}>{permission.name}</span>
                      {permission.description && (
                        <span style={{ color: '#666', marginLeft: '8px' }}>
                          - {permission.description}
                        </span>
                      )}
                    </Checkbox>
                  </div>
                ))}
              </div>
            </Collapse.Panel>
          ))}
        </Collapse>
      )}
    </Modal>
  );
};
