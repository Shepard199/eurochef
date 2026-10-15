import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;

public class RobotsInputGetter2220Evidence extends GhidraScript {
  public void run() throws Exception {
    Address a = toAddr(0x004d2220L);
    disassemble(a);
    Function f = getFunctionAt(a);
    if (f == null) {
      try { f = createFunction(a, null); }
      catch (Exception e) { println("CREATE_FAIL " + e.getMessage()); }
    }
    println("FUNC=" + (f == null ? "<none>" : f.getName()+" @ "+f.getEntryPoint()+" BODY="+f.getBody()));
    InstructionIterator it = currentProgram.getListing().getInstructions(a, true);
    int n = 0;
    while (it.hasNext() && n++ < 64) {
      Instruction ins = it.next();
      println(ins.getAddress()+"  "+ins);
      if (ins.getFlowType().isTerminal()) break;
    }
    if (f != null) {
      DecompInterface d = new DecompInterface();
      d.openProgram(currentProgram);
      DecompileResults r = d.decompileFunction(f, 90, monitor);
      if (r.decompileCompleted() && r.getDecompiledFunction() != null) println(r.getDecompiledFunction().getC());
      else println("DECOMP_ERR="+r.getErrorMessage());
      d.dispose();
    }
  }
}