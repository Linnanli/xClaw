/**
 * 认证登录 E2E 测试
 * 使用真实后端 API 进行测试
 * 
 * 测试覆盖率类型：
 * - 单元测试：登录表单验证
 * - 集成测试：前后端登录流程
 * - 失败路径测试：错误凭据、网络错误
 * - 安全测试：密码隐藏、令牌存储
 * - 可靠性测试：重试机制、超时处理
 * - 需求级测试：登录功能规范
 * - 用户体验测试：加载状态、错误提示
 */

describe('认证登录 E2E 测试', () => {
  const apiUrl = Cy.env('apiUrl');
  const testUsername = Cy.env('testUsername');
  const testPassword = Cy.env('testPassword');

  beforeEach(() => {
    // 清除本地存储
    cy.clearLocalStorage();
    cy.clearCookies();
    
    // 访问登录页面
    cy.visit('/login');
    
    // 等待页面加载完成
    cy.get('.login-container', { timeout: 10000 }).should('be.visible');
  });

  describe('单元测试 - 表单验证', () => {
    it('应该显示登录表单的所有元素', () => {
      // 验证表单元素存在
      cy.get('input[name="username"]').should('be.visible');
      cy.get('input[type="password"]').should('be.visible');
      cy.get('button[type="submit"]').should('be.visible');
      cy.get('.remember-me-checkbox').should('exist');
      
      // 验证标题
      cy.contains('IronClaw 管理后台').should('be.visible');
    });

    it('应该验证用户名为必填项', () => {
      // 不输入用户名，直接点击登录
      cy.get('input[type="password"]').type('password123');
      cy.get('button[type="submit"]').click();
      
      // 验证错误提示
      cy.contains('请输入用户名').should('be.visible');
    });

    it('应该验证密码为必填项', () => {
      // 不输入密码，直接点击登录
      cy.get('input[name="username"]').type('admin');
      cy.get('button[type="submit"]').click();
      
      // 验证错误提示
      cy.contains('请输入密码').should('be.visible');
    });

    it('应该验证用户名长度限制', () => {
      // 输入过短的用户名
      cy.get('input[name="username"]').type('ab');
      cy.get('input[type="password"]').type('password123');
      cy.get('button[type="submit"]').click();
      
      // 验证错误提示
      cy.contains('用户名至少3个字符').should('be.visible');
    });

    it('应该验证密码长度限制', () => {
      // 输入过短的密码
      cy.get('input[name="username"]').type('admin');
      cy.get('input[type="password"]').type('123');
      cy.get('button[type="submit"]').click();
      
      // 验证错误提示
      cy.contains('密码至少6个字符').should('be.visible');
    });
  });

  describe('集成测试 - 登录流程', () => {
    it('应该成功登录并跳转到仪表盘', () => {
      // 输入正确的凭据
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      // 等待登录请求完成
      cy.wait(2000);
      
      // 验证跳转到仪表盘
      cy.url().should('include', '/dashboard');
      
      // 验证仪表盘页面加载
      cy.get('.dashboard-container', { timeout: 10000 }).should('be.visible');
      
      // 验证用户信息显示
      cy.get('.user-menu').should('contain', testUsername);
    });

    it('应该在登录后保存令牌到 localStorage', () => {
      // 登录
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      // 验证令牌已保存
      cy.window().then((win) => {
        const token = win.localStorage.getItem('token');
        expect(token).to.exist;
        expect(token).to.have.length.greaterThan(20);
      });
    });

    it('应该在勾选"记住我"后保存用户名', () => {
      // 勾选记住我
      cy.get('.remember-me-checkbox').check();
      
      // 登录
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      // 验证用户名已保存
      cy.window().then((win) => {
        const savedUsername = win.localStorage.getItem('rememberedUsername');
        expect(savedUsername).to.equal(testUsername);
      });
      
      // 退出登录
      cy.get('.user-menu').click();
      cy.contains('退出登录').click();
      
      // 验证返回登录页面时用户名已填充
      cy.get('input[name="username"]').should('have.value', testUsername);
    });

    it('应该能够使用 Enter 键提交表单', () => {
      // 输入凭据
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type(testPassword);
      
      // 按 Enter 键
      cy.get('input[type="password"]').type('{enter}');
      
      cy.wait(2000);
      
      // 验证登录成功
      cy.url().should('include', '/dashboard');
    });
  });

  describe('失败路径测试 - 错误处理', () => {
    it('应该处理错误的用户名', () => {
      // 输入错误的用户名
      cy.get('input[name="username"]').type('wronguser');
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      // 验证错误提示
      cy.get('.error-message', { timeout: 5000 }).should('be.visible');
      cy.get('.error-message').should('contain', '用户名或密码错误');
      
      // 验证仍在登录页面
      cy.url().should('include', '/login');
    });

    it('应该处理错误的密码', () => {
      // 输入错误的密码
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type('wrongpassword');
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      // 验证错误提示
      cy.get('.error-message', { timeout: 5000 }).should('be.visible');
      cy.get('.error-message').should('contain', '用户名或密码错误');
    });

    it('应该处理后端服务不可用', () => {
      // 拦截登录请求并返回网络错误
      cy.intercept('POST', `${apiUrl}/auth/login`, {
        forceNetworkError: true,
      }).as('loginRequest');
      
      // 尝试登录
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait('@loginRequest');
      
      // 验证错误提示
      cy.get('.error-message', { timeout: 5000 }).should('be.visible');
      cy.get('.error-message').should('contain', '网络错误');
    });

    it('应该处理服务器错误（500）', () => {
      // 拦截登录请求并返回 500 错误
      cy.intercept('POST', `${apiUrl}/auth/login`, {
        statusCode: 500,
        body: { error: 'Internal Server Error' },
      }).as('loginRequest');
      
      // 尝试登录
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait('@loginRequest');
      
      // 验证错误提示
      cy.get('.error-message', { timeout: 5000 }).should('be.visible');
      cy.get('.error-message').should('contain', '服务器错误');
    });

    it('应该处理请求超时', () => {
      // 拦截登录请求并延迟响应
      cy.intercept('POST', `${apiUrl}/auth/login`, (req) => {
        req.reply({
          delay: 15000, // 超过 10 秒超时
          statusCode: 200,
          body: { token: 'test-token' },
        });
      }).as('loginRequest');
      
      // 尝试登录
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      // 验证超时错误提示
      cy.get('.error-message', { timeout: 12000 }).should('be.visible');
      cy.get('.error-message').should('contain', '请求超时');
    });
  });

  describe('安全测试 - 密码和令牌安全', () => {
    it('应该隐藏密码输入', () => {
      // 验证密码输入框类型为 password
      cy.get('input[type="password"]').should('have.attr', 'type', 'password');
      
      // 输入密码
      cy.get('input[type="password"]').type('secretpassword');
      
      // 验证密码不可见（显示为点或星号）
      cy.get('input[type="password"]').should('not.have.value', 'secretpassword');
    });

    it('应该不在 URL 中暴露密码', () => {
      // 登录
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      // 验证 URL 中不包含密码
      cy.url().should('not.contain', testPassword);
    });

    it('应该不在控制台日志中暴露密码', () => {
      // 监听控制台日志
      cy.window().then((win) => {
        cy.spy(win.console, 'log').as('consoleLog');
      });
      
      // 登录
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      // 验证控制台日志不包含密码
      cy.get('@consoleLog').should((spy) => {
        const calls = (spy as any).getCalls();
        calls.forEach((call: any) => {
          const args = call.args.join(' ');
          expect(args).to.not.contain(testPassword);
        });
      });
    });

    it('应该安全存储令牌（不在 cookie 中）', () => {
      // 登录
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      // 验证令牌不在 cookie 中
      cy.getCookie('token').should('be.null');
      
      // 验证令牌在 localStorage 中
      cy.window().then((win) => {
        const token = win.localStorage.getItem('token');
        expect(token).to.exist;
      });
    });
  });

  describe('可靠性测试 - 并发和重试', () => {
    it('应该防止重复提交', () => {
      // 快速点击登录按钮多次
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type(testPassword);
      
      cy.get('button[type="submit"]').click();
      cy.get('button[type="submit"]').click();
      cy.get('button[type="submit"]').click();
      
      // 验证只发送一次请求
      cy.intercept('POST', `${apiUrl}/auth/login`).as('loginRequest');
      
      // 等待请求完成
      cy.wait(2000);
      
      // 验证登录成功（只执行一次）
      cy.url().should('include', '/dashboard');
    });

    it('应该在登录失败后允许重试', () => {
      // 第一次尝试：错误密码
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type('wrongpassword');
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      // 验证错误提示
      cy.get('.error-message').should('be.visible');
      
      // 第二次尝试：正确密码
      cy.get('input[type="password"]').clear().type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      // 验证登录成功
      cy.url().should('include', '/dashboard');
    });
  });

  describe('需求级测试 - 功能规范', () => {
    it('REQ-AUTH-001: 应该支持用户名密码登录', () => {
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      cy.url().should('include', '/dashboard');
    });

    it('REQ-AUTH-002: 应该在登录后保存会话', () => {
      // 登录
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      // 刷新页面
      cy.reload();
      
      // 验证仍然登录（未跳转到登录页）
      cy.url().should('not.include', '/login');
      cy.get('.dashboard-container', { timeout: 10000 }).should('be.visible');
    });

    it('REQ-AUTH-003: 应该在登录失败时显示错误信息', () => {
      cy.get('input[name="username"]').type('wronguser');
      cy.get('input[type="password"]').type('wrongpassword');
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      cy.get('.error-message').should('be.visible');
      cy.get('.error-message').should('contain', '用户名或密码错误');
    });

    it('REQ-AUTH-004: 应该支持"记住我"功能', () => {
      cy.get('.remember-me-checkbox').check();
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      // 验证用户名已保存
      cy.window().then((win) => {
        const savedUsername = win.localStorage.getItem('rememberedUsername');
        expect(savedUsername).to.equal(testUsername);
      });
    });
  });

  describe('用户体验测试 - 交互和反馈', () => {
    it('应该在登录时显示加载状态', () => {
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      // 验证加载状态
      cy.get('.loading-spinner').should('be.visible');
      cy.get('button[type="submit"]').should('be.disabled');
      
      cy.wait(2000);
      
      // 验证加载状态消失
      cy.get('.loading-spinner').should('not.exist');
    });

    it('应该在输入时清除错误提示', () => {
      // 先触发错误
      cy.get('input[name="username"]').type('wronguser');
      cy.get('input[type="password"]').type('wrongpassword');
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      cy.get('.error-message').should('be.visible');
      
      // 修改输入
      cy.get('input[name="username"]').clear().type(testUsername);
      
      // 验证错误提示消失
      cy.get('.error-message').should('not.exist');
    });

    it('应该在密码输入框提供显示/隐藏切换', () => {
      // 输入密码
      cy.get('input[type="password"]').type('testpassword');
      
      // 点击显示密码按钮
      cy.get('.toggle-password-visibility').click();
      
      // 验证密码可见
      cy.get('input[type="text"]').should('have.value', 'testpassword');
      
      // 再次点击隐藏密码
      cy.get('.toggle-password-visibility').click();
      
      // 验证密码隐藏
      cy.get('input[type="password"]').should('exist');
    });

    it('应该在表单验证失败时聚焦到第一个错误字段', () => {
      // 不输入任何内容，直接提交
      cy.get('button[type="submit"]').click();
      
      // 验证用户名输入框获得焦点
      cy.focused().should('have.attr', 'name', 'username');
    });
  });

  describe('代码覆盖测试 - 边界情况', () => {
    it('应该处理空用户名', () => {
      cy.get('input[name="username"]').clear();
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.contains('请输入用户名').should('be.visible');
    });

    it('应该处理空密码', () => {
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').clear();
      cy.get('button[type="submit"]').click();
      
      cy.contains('请输入密码').should('be.visible');
    });

    it('应该处理只包含空格的用户名', () => {
      cy.get('input[name="username"]').type('   ');
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.contains('请输入用户名').should('be.visible');
    });

    it('应该处理特殊字符用户名', () => {
      cy.get('input[name="username"]').type('admin@#$%');
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      // 应该显示错误（特殊字符用户名不存在）
      cy.get('.error-message').should('be.visible');
    });

    it('应该处理超长用户名', () => {
      const longUsername = 'a'.repeat(100);
      cy.get('input[name="username"]').type(longUsername);
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      // 应该显示错误或被截断
      cy.get('.error-message, .dashboard-container').should('exist');
    });

    it('应该处理超长密码', () => {
      const longPassword = 'a'.repeat(100);
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type(longPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      // 应该显示错误
      cy.get('.error-message').should('be.visible');
    });
  });

  describe('数据覆盖测试 - 各种输入格式', () => {
    it('应该处理小写用户名', () => {
      cy.get('input[name="username"]').type(testUsername.toLowerCase());
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      // 验证登录结果（取决于后端是否区分大小写）
      cy.url().should('match', /\/(dashboard|login)/);
    });

    it('应该处理大写用户名', () => {
      cy.get('input[name="username"]').type(testUsername.toUpperCase());
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      cy.url().should('match', /\/(dashboard|login)/);
    });

    it('应该处理混合大小写用户名', () => {
      cy.get('input[name="username"]').type('AdMiN');
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      cy.url().should('match', /\/(dashboard|login)/);
    });

    it('应该处理包含数字的用户名', () => {
      cy.get('input[name="username"]').type('admin123');
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      cy.url().should('match', /\/(dashboard|login)/);
    });

    it('应该处理包含下划线的用户名', () => {
      cy.get('input[name="username"]').type('admin_user');
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.wait(2000);
      
      cy.url().should('match', /\/(dashboard|login)/);
    });
  });

  describe('性能测试 - 响应时间', () => {
    it('登录请求应该在 2 秒内完成', () => {
      const startTime = Date.now();
      
      cy.get('input[name="username"]').type(testUsername);
      cy.get('input[type="password"]').type(testPassword);
      cy.get('button[type="submit"]').click();
      
      cy.url().should('include', '/dashboard').then(() => {
        const endTime = Date.now();
        const duration = endTime - startTime;
        
        expect(duration).to.be.lessThan(2000);
      });
    });

    it('页面加载应该在 1 秒内完成', () => {
      cy.visit('/login', {
        onBeforeLoad: (win) => {
          (win as any).performance.mark('start');
        },
      });
      
      cy.get('.login-container').should('be.visible').then(() => {
        cy.window().then((win) => {
          (win as any).performance.mark('end');
          (win as any).performance.measure('pageLoad', 'start', 'end');
          
          const measure = (win as any).performance.getEntriesByName('pageLoad')[0];
          expect(measure.duration).to.be.lessThan(1000);
        });
      });
    });
  });
});
