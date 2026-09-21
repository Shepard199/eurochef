import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.scalar.Scalar;
import java.util.*;
public class CurrentAttackerRangeEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  long lo=0x007B2A00L, hi=0x007B2A18L;
  Set<Function> fs=new LinkedHashSet<>();
  InstructionIterator it=currentProgram.getListing().getInstructions(true);
  while(it.hasNext()){
   Instruction ins=it.next();
   boolean hit=false;
   for(int op=0;op<ins.getNumOperands();op++) for(Object obj:ins.getOpObjects(op)){
    long v=-1;
    if(obj instanceof Scalar) v=((Scalar)obj).getUnsignedValue();
    else if(obj instanceof Address) v=((Address)obj).getOffset();
    if(v>=lo&&v<=hi) hit=true;
   }
   if(hit){
    Function f=getFunctionContaining(ins.getAddress());
    println(ins.getAddress()+" "+ins+" "+(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
    if(f!=null)fs.add(f);
   }
  }
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  for(Function f:fs){println("\n=== "+f.getName()+" "+f.getEntryPoint()+" ===");DecompileResults r=d.decompileFunction(f,90,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());}
  d.dispose();
 }
}