import './commands'

Cypress.on('uncaught:exception', (err) => {
  if (err.message.includes('ResizeObserver')) return false
  return true
})
