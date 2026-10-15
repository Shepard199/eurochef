import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.address.Address;
public class InputManagerCtorEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void dec(long x)throws Exception{Function f=getFunctionContaining(toAddr(x));println(String.format("\n=== 0x%08X ===",x));if(f==null){println("NO_FUNCTION");return;}println(f.getName()+" @ "+f.getEntryPoint());DecompileResults r=d.decompileFunction(f,60,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());}
 public void run()throws Exception{d.openProgram(currentProgram);long[] a={0x004f89d3L,0x004f8a0cL,0x004f8a20L,0x004f8b00L,0x0052432fL,0x00528fafL};for(long x:a)dec(x);d.dispose();}
}