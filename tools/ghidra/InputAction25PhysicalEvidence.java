import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
public class InputAction25PhysicalEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void dec(long x)throws Exception{Function f=getFunctionContaining(toAddr(x));println(String.format("\n=== DEC 0x%08X ===",x));if(f==null){println("NO_FUNCTION");return;}println("FUNCTION="+f.getName()+" ENTRY="+f.getEntryPoint());DecompileResults r=d.decompileFunction(f,60,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());}
 void refsData(long x)throws Exception{println(String.format("\n=== REFS DATA 0x%08X ===",x));ReferenceIterator it=currentProgram.getReferenceManager().getReferencesTo(toAddr(x));while(it.hasNext()){Reference r=it.next();Function f=getFunctionContaining(r.getFromAddress());println(r.getReferenceType()+" "+r.getFromAddress()+" FROM "+(f==null?"<none>":f.getName()+"@"+f.getEntryPoint()));}}
 public void run()throws Exception{d.openProgram(currentProgram);long[] a={0x00508e3fL,0x00507cf8L};for(long x:a)dec(x);refsData(0x007be1b0L);d.dispose();}
}